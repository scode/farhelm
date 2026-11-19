#!/usr/bin/env python3
"""The recorder's frame, cut and output contracts, without a live app or ffmpeg."""

from pathlib import Path
import tempfile
import unittest

import readme_video as video


class RecordingTests(unittest.TestCase):
    def test_a_cut_excludes_the_wait_and_each_mark_names_its_own_frame(self):
        """A slow first segment and faster second one retain their pace without inserting time between them."""
        first = tuple(Path(f"/a/frame-{index}.png") for index in range(3))
        second = tuple(Path(f"/b/frame-{index}.png") for index in range(2))
        manifest, marks, seconds = video.timeline([
            video.Segment(first, 1.5, (video.Mark("Opening", 1),)),
            video.Segment(second, 0.5, (video.Mark("After the cut", 0),)),
        ])
        self.assertEqual(marks, [("Opening", 0.5, first[1]), ("After the cut", 1.5, second[0])])
        self.assertEqual(seconds, 2.0)
        self.assertEqual(manifest.splitlines().count("duration 0.500000000"), 3)
        self.assertEqual(manifest.splitlines().count("duration 0.250000000"), 2)
        self.assertEqual(manifest.count(video.concat_file(second[-1])), 2)

    def test_an_async_frame_writer_must_finish_before_capture_can_use_it(self):
        """An observed filename can precede frame durability. The trailer distinguishes that interval."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "frame-00000.png"
            self.assertFalse(video.complete_png(path))
            path.write_bytes(b"\x89PNG\r\n\x1a\n")
            self.assertFalse(video.complete_png(path))
            with path.open("ab") as stream:
                stream.write(b"\x00\x00\x00\x00IEND\xaeB\x60\x82")
            self.assertTrue(video.complete_png(path))

    def test_the_encoder_preserves_the_marked_frames_and_applies_the_crop_once(self):
        """A constant-rate resample could drop a marked frame, leaving a still absent from the movie."""
        command = video.encode_command(Path("/input list"), Path("/out.mp4"), (21, 55), (1920, 1080))
        self.assertEqual(command.count("-i"), 1)
        self.assertEqual(command[command.index("-vf") + 1], "crop=1920:1080:21:55")
        self.assertEqual(command[command.index("-fps_mode") + 1], "vfr")
        self.assertEqual(command[command.index("-enc_time_base") + 1], "1:1000")
        self.assertNotIn("-r", command)
        self.assertEqual(command[command.index("-c:v") + 1], "libx264")
        self.assertEqual(video.concat_file(Path("/run'one/frame.png")), r"file '/run'\''one/frame.png'")

    def test_only_stills_accounted_for_in_existing_marks_can_be_replaced(self):
        """An output basename can collide with a directory someone else created. Leave unknown files alone."""
        with tempfile.TemporaryDirectory() as directory:
            movie = Path(directory) / "test.mp4"
            marks, stills = video.output_paths(movie)
            stills.mkdir()
            image = stills / "01-opening.png"
            image.touch()
            with self.assertRaisesRegex(RuntimeError, "not accounted for"):
                video.check_output(movie, marks, stills)
            marks.write_text('[{"still":"test-stills/01-opening.png"}]')
            video.check_output(movie, marks, stills)
            (stills / "other").touch()
            with self.assertRaisesRegex(RuntimeError, "not accounted for"):
                video.check_output(movie, marks, stills)

    def test_a_directory_at_the_movie_path_is_refused(self):
        """A destination the final rename cannot replace should fail before a capture starts."""
        with tempfile.TemporaryDirectory() as directory:
            movie = Path(directory) / "test.mp4"
            movie.mkdir()
            with self.assertRaisesRegex(RuntimeError, "expected a regular file"):
                video.check_output(movie, *video.output_paths(movie))

    def test_failed_install_restores_the_previous_capture(self):
        """Stills, marks and the movie must keep describing the same capture after a partial install fails."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            movie = root / "previous.mp4"
            marks, stills = video.output_paths(movie)
            movie.write_bytes(b"old movie")
            stills.mkdir()
            previous_marks = '[{"still":"previous-stills/old.png"}]'
            marks.write_text(previous_marks)
            (stills / "old.png").write_bytes(b"old image")
            staged, backup = root / "new", root / "old"
            backup.mkdir()
            staged_stills = staged / stills.name
            staged_stills.mkdir(parents=True)
            (staged_stills / "new.png").write_bytes(b"new image")
            (staged / marks.name).write_text('[{"still":"previous-stills/new.png"}]')

            with self.assertRaises(FileNotFoundError):
                video.install_output(movie, marks, stills, staged, backup)
            self.assertEqual(movie.read_bytes(), b"old movie")
            self.assertEqual(marks.read_text(), previous_marks)
            self.assertEqual([path.name for path in stills.iterdir()], ["old.png"])
            self.assertFalse(list(backup.iterdir()))

            (staged / movie.name).write_bytes(b"new movie")
            video.install_output(movie, marks, stills, staged, backup)
            self.assertEqual(movie.read_bytes(), b"new movie")
            self.assertEqual([path.name for path in stills.iterdir()], ["new.png"])
            video.check_output(movie, marks, stills)


if __name__ == "__main__":
    unittest.main()
