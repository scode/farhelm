//! Exercise the model's view of the Farhelm instructions pointer with both settings. Conversation capture must
//! still work when the pointer is omitted.

use super::*;
use farhelm_supervisor::agent_kind::{AgentInstructions, delivers_instructions};

/// Use the same request in both launches. State the framing and negative answer separately, so even a wrapped
/// prompt echo cannot supply a complete reply. The end marker keeps a streamed prefix from counting as an answer.
const QUESTION: &str = "Do not call tools. From the instructions already in context, give only the command associated with $farhelm or NONE if absent. Begin with FH and a colon. Finish with a colon and END.";
const COMMAND: &str = "farhelm agent instructions";
const MISSING: &str = "NONE";

/// Give both settings a chance to produce evidence. A wrong answer in the enabled run must not hide an
/// independent loss of conversation capture in the disabled run.
async fn execute(harness: LaunchHarness) -> DriftResult {
    let profile = profile(harness).expect("an instructions case has a drift profile");
    let mut failures = Vec::new();
    let mut unavailable = Vec::new();
    for (instructions, reply) in [
        (AgentInstructions::On, COMMAND),
        (AgentInstructions::Off, MISSING),
    ] {
        match check_instruction_policy(harness, &profile, instructions, reply).await {
            Ok(()) => {
                println!("instructions {instructions:?}: matching reply and accepted capture")
            }
            Err(DriftError::CouldNotRun(reason)) => {
                unavailable.push(format!("{instructions:?}: {reason}"))
            }
            Err(DriftError::Failed(error)) => failures.push(format!("{instructions:?}: {error:#}")),
        }
    }
    if !failures.is_empty() {
        failures.extend(unavailable);
        return Err(anyhow::anyhow!("{}", failures.join("\n")).into());
    }
    if !unavailable.is_empty() {
        return Err(DriftError::CouldNotRun(unavailable.join("\n")));
    }
    Ok(())
}

/// A new supervisor and workspace keep the disabled launch from inheriting either a conversation or a startup
/// pointer from the enabled launch. The two use the same compiled agent choices.
async fn check_instruction_policy(
    harness: LaunchHarness,
    profile: &Harness,
    instructions: AgentInstructions,
    reply: &str,
) -> DriftResult {
    let selection = ordinary_selection(harness, profile);
    let seams = SupervisorSeams {
        agent_instructions: instructions,
        ..Default::default()
    };
    let hook_witness = profile
        .hook_witness
        .expect("an instructions case also exercises reporting");
    with_drift_supervisor_using(profile, selection, None, seams, async |env, launch| {
        env.with_session(launch, farhelm_helm::CreateExtras::default(), async |id| {
            // Leave room for the full input and the vendor's composer decorations on one line.
            let mut terminal =
                Terminal::attach_with_width(env.sup, env.client, id, false, 200).await?;
            terminal
                .ready(profile, TrustPolicy::AutoAnswer, false)
                .await?;
            let expected = TurnExpectation {
                reply,
                acked_before: 0,
                original_argv: None,
                hook_witness,
            };
            if framed_reply(&terminal.host.screen_tail(SCREEN_BYTES)).is_some() {
                return Err(
                    anyhow::anyhow!("the reply witness was visible before the question").into(),
                );
            }
            terminal.type_text(QUESTION, false).await?;
            terminal
                .client
                .send_keys(terminal.channel, b"\r".to_vec())
                .await;
            let mut answer = None;
            wait_for_turn_matching(
                &mut terminal,
                env.sup,
                id,
                &env.state.join("hook-log").join(format!("{id}.log")),
                &expected,
                |text| {
                    if let Some(body) = framed_reply(text) {
                        answer = Some(body);
                    }
                    answer.is_some()
                },
            )
            .await?;
            let answer = answer.expect("the turn waiter found a complete framed reply");
            if !matches_answer(&answer, reply) {
                return Err(anyhow::anyhow!(
                    "the model named a different command or reported the wrong pointer state\n{}",
                    terminal.diagnostics()
                )
                .into());
            }
            if !instructions.announces() {
                let screen = terminal.host.screen_tail(SCREEN_BYTES).to_ascii_lowercase();
                let text = screen.split_whitespace().collect::<Vec<_>>().join(" ");
                if text.contains(COMMAND) {
                    return Err(anyhow::anyhow!(
                        "the disabled launch still showed the instructions command"
                    )
                    .into());
                }
            }
            Ok(())
        })
        .await
    })
    .await
}

/// Read the last complete framed reply in a single screen snapshot. The command and markers can wrap, and
/// a reply can end in a period after the closing marker. The question contains neither complete answer.
fn framed_reply(screen: &str) -> Option<String> {
    let mut complete = None;
    let mut body = None;
    for line in screen.lines().map(str::trim) {
        let line = line
            .trim_start_matches(['•', '●', '✦', '◆', '-', '*'])
            .trim_start();
        let line = if let Some(rest) = line
            .strip_prefix("FH")
            .and_then(|rest| rest.trim_start().strip_prefix(':'))
        {
            body = Some(String::new());
            rest.trim_start()
        } else {
            line
        };
        let Some(current) = &mut body else {
            continue;
        };
        let closing = line.strip_suffix('.').unwrap_or(line).trim_end();
        let end = closing
            .strip_suffix("END")
            .and_then(|rest| rest.trim_end().strip_suffix(':'))
            .map(str::trim_end);
        let part = end.unwrap_or(line);
        if !part.is_empty() {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(part);
        }
        if end.is_some() {
            complete = body.take();
        }
    }
    complete
}

/// Accept display quoting around the expected answer. Merely mentioning the command in prose could include a
/// denial, so the payload has to name the exact command or give the complete negative answer.
fn matches_answer(body: &str, expected: &str) -> bool {
    let body = body.trim().trim_matches(['`', '\'', '"', '*']).trim();
    body.split_whitespace().eq(expected.split_whitespace())
}

drift_cases! {
    execute;
    drift_claude => Claude,
    drift_codex => Codex,
    drift_muse => Muse,
    drift_goose => Goose,
    drift_pi => Pi,
    drift_omp => Omp,
}

/// Every launch integration that toggles a pointer needs exactly one live case that exercises both settings.
#[farhelm_testtrace::test]
fn instructions_delivery_covers_every_pointer_harness() {
    let expected: Vec<_> = LaunchHarness::BUILT_INS
        .iter()
        .copied()
        .filter(|harness| delivers_instructions(harness.agent_kind()))
        .collect();
    assert_eq!(HARNESSES.len(), expected.len());
    for harness in expected {
        assert_eq!(
            HARNESSES.iter().filter(|&&case| case == harness).count(),
            1,
            "{harness:?}"
        );
    }
    assert!(!QUESTION.contains(COMMAND));
    assert!(!QUESTION.contains(&format!("FH:{MISSING}:END")));
    assert!(framed_reply(QUESTION).is_none());
}

/// Display wrapping and Markdown quoting can preserve a command. Echoes, unfinished answers and denials
/// cannot supply the same evidence.
#[farhelm_testtrace::test]
fn the_pointer_witness_requires_a_complete_framed_answer() {
    let wrapped = framed_reply("● FH: `farhelm agent\ninstructions` :END").expect("complete frame");
    assert!(matches_answer(&wrapped, COMMAND));
    let negative = framed_reply("◆ FH:\n NONE\n :END").expect("complete frame");
    assert!(matches_answer(&negative, MISSING));
    let adjacent = framed_reply("FH:farhelm agent instructions:END").expect("complete frame");
    assert!(matches_answer(&adjacent, COMMAND));
    let punctuated = framed_reply("FH: farhelm agent instructions: END.").expect("complete frame");
    assert!(matches_answer(&punctuated, COMMAND));
    assert!(framed_reply("FH:NONE:END. More text follows.").is_none());
    assert!(framed_reply("› FH:farhelm agent instructions:END").is_none());
    assert!(
        framed_reply("if absent. Begin with\nFH and a colon. Finish with\na colon and END.")
            .is_none()
    );
    assert!(framed_reply("◆ FH:NONE").is_none());
    assert!(!matches_answer(
        "I was not told to run farhelm agent instructions",
        COMMAND
    ));
    assert!(!matches_answer(
        "NONE, but run farhelm agent instructions",
        MISSING
    ));
}
