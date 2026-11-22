# The CentOS gate checks a relative artifact path from the caller’s directory

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The CentOS gate can verify the wrong file when Cargo output is relative and the command starts outside the checkout.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d scripts/test-provision-centos.sh:48,60–65,258–271,493–502;
scripts/check-static-elf.sh:48–58; crates/farhelm-helm/src/provisioning.rs:13049–13057

A caller outside the checkout using a relative CARGO_TARGET_DIR can get a missing-file failure after Cargo successfully
creates the binary. The script sets repo using a command substitution and retains the target path as given. Cargo builds
after cd to repo in a subshell. Once the subshell exits, the static checker opens the quoted relative path from the
original caller’s directory; it does not change directory. The later nextest wrapper enters repo and passes the original
relative string through FARHELM_TEST_BINARY. The Rust test creates a PathBuf without resolving it to the checkout or
recording the previously checked bytes. The checker’s mismatch follows from shell directory scoping before nextest; no
assumption about nextest’s per-test current directory is needed. If a separate file exists at the checker location, its
passing ELF shape does not establish that the test opened those bytes; presence of such a file was not verified. An
absolute external target or the checkout-absolute default avoids this directory mismatch. The reviewer suggested other,
confirmed by source, , Possible coverage: related to trl-09 but distinct trigger.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-12`.
