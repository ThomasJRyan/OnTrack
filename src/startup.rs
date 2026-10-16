//! Cooked-terminal setup, completed before raw mode or the startup splash.
use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
    process::{Command, Output, Stdio},
};

pub fn ensure_workspace() -> io::Result<bool> {
    let directory = std::env::current_dir()?;
    let result = setup(
        &directory,
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        |args, interactive| {
            let mut command = Command::new("bd");
            command.args(args).current_dir(&directory);
            if interactive {
                command
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());
            }
            command.output()
        },
    );
    result.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Beads startup in {}: {error}. Check bd and workspace permissions, then retry",
                directory.display()
            ),
        )
    })
}

fn setup(
    directory: &Path,
    interactive: bool,
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut execute: impl FnMut(&[String], bool) -> io::Result<Output>,
) -> io::Result<bool> {
    let discovery = execute(&strings(&["--no-color", "where", "--json"]), false)?;
    if workspace_exists(&discovery)? {
        return Ok(true);
    }
    if !interactive {
        return Err(io::Error::other(
            "No Beads workspace found; run OnTrack in a terminal for setup, or run bd init first",
        ));
    }
    let help = checked(
        execute(&strings(&["--no-color", "init", "--help"]), false)?,
        "read bd init options",
    )?;
    let options = init_options(&String::from_utf8_lossy(&help));
    writeln!(
        output,
        "OnTrack — set up Beads in {}\nNo workspace was found. Enter accepts the shown default; :cancel or Ctrl+D cancels.\nNothing is written until you confirm.\n",
        directory.display()
    )?;
    let Some(plan) = dialogue(input, output, &options)? else {
        writeln!(output, "Setup cancelled.")?;
        return Ok(false);
    };
    // Native contributor/team wizards, when selected, retain access to the terminal.
    checked(
        execute(&plan.args, true)?,
        "initialize Beads (run bd init to inspect or retry)",
    )?;
    checked(
        execute(
            &strings(&[
                "--no-color",
                "config",
                "set",
                "export.auto",
                if plan.export { "true" } else { "false" },
            ]),
            false,
        )?,
        "save export.auto after initialization; the workspace may already exist, use bd config set export.auto to retry",
    )?;
    writeln!(
        output,
        "Beads is ready. OnTrack preferences (theme, default agent, agent limit and cleanup) are available with s in the browser."
    )?;
    Ok(true)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn checked(output: Output, step: &str) -> io::Result<Vec<u8>> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(io::Error::other(format!(
            "Could not {step} ({}): {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        )))
    }
}

fn workspace_exists(output: &Output) -> io::Result<bool> {
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok();
    if output.status.success() {
        // Discovery belongs to bd, including ancestors, redirects and worktrees.
        if value
            .as_ref()
            .and_then(|value| value["path"].as_str())
            .is_some_and(|path| !path.trim().is_empty())
        {
            return Ok(true);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bd where returned invalid workspace JSON; expected a nonempty path",
        ));
    }
    if value.as_ref().and_then(|value| value["error"].as_str()) == Some("no_beads_directory") {
        return Ok(false);
    }
    Err(io::Error::other(format!(
        "Could not discover Beads workspace: {} {}",
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    )))
}

#[derive(Debug)]
struct InitOption {
    name: String,
    kind: Option<String>,
    help: String,
}

/// Read only init's flags, never global flags or destructive recovery switches.
fn init_options(help: &str) -> Vec<InitOption> {
    let mut in_flags = false;
    let mut options = Vec::new();
    for line in help.lines() {
        if line == "Flags:" {
            in_flags = true;
            continue;
        }
        if line == "Global Flags:" {
            break;
        }
        if !in_flags {
            continue;
        }
        let Some(start) = line.find("--") else {
            continue;
        };
        let flag = &line[start..];
        let Some(split) = flag.find("  ") else {
            continue;
        };
        let mut fields = flag[..split].split_whitespace();
        let name = fields.next().unwrap().trim_start_matches("--");
        if matches!(
            name,
            "force"
                | "reinit-local"
                | "destroy-token"
                | "discard-remote"
                | "from-jsonl"
                | "help"
                | "quiet"
                | "non-interactive"
                | "init-if-missing"
                | "backend"
        ) {
            continue;
        }
        options.push(InitOption {
            name: name.into(),
            kind: fields.next().map(str::to_owned),
            help: flag[split..].trim().into(),
        });
    }
    options
}

struct Plan {
    args: Vec<String>,
    export: bool,
}

fn ask(
    input: &mut impl BufRead,
    output: &mut impl Write,
    label: &str,
    default: &str,
) -> io::Result<Option<String>> {
    loop {
        write!(output, "{label} [{default}]: ")?;
        output.flush()?;
        let mut value = String::new();
        if input.read_line(&mut value)? == 0 {
            return Ok(None);
        }
        let value = value.trim();
        if value == ":cancel" {
            return Ok(None);
        }
        if value.chars().any(char::is_control) {
            writeln!(output, "Control characters are not allowed.")?;
            continue;
        }
        return Ok(Some(if value.is_empty() {
            default.into()
        } else {
            value.into()
        }));
    }
}

fn choose(
    input: &mut impl BufRead,
    output: &mut impl Write,
    label: &str,
    default: &str,
    choices: &[&str],
) -> io::Result<Option<String>> {
    loop {
        let Some(value) = ask(input, output, label, default)? else {
            return Ok(None);
        };
        if choices.contains(&value.as_str()) {
            return Ok(Some(value));
        }
        writeln!(output, "Choose {}.", choices.join(" / "))?;
    }
}

fn dialogue(
    input: &mut impl BufRead,
    output: &mut impl Write,
    options: &[InitOption],
) -> io::Result<Option<Plan>> {
    // Cancellation propagates without running any mutating commands.
    macro_rules! answer {
        ($value:expr) => {
            match $value? {
                Some(value) => value,
                None => return Ok(None),
            }
        };
    }
    let prefix = answer!(ask(
        input,
        output,
        "Issue prefix (empty uses directory name)",
        ""
    ));
    let role = answer!(choose(
        input,
        output,
        "Role: maintainer / contributor",
        "maintainer",
        &["maintainer", "contributor"]
    ));
    let storage = answer!(choose(
        input,
        output,
        "Storage: embedded / server / shared-server",
        "embedded",
        &["embedded", "server", "shared-server"]
    ));
    let mut args = strings(&["--no-color", "init"]);
    if !prefix.is_empty() {
        args.push(format!("--prefix={prefix}"));
    }
    args.push(format!("--role={role}"));
    if storage != "embedded" {
        args.push(format!("--{storage}"));
    }
    if storage == "server" {
        for (flag, label, default) in [
            ("server-host", "Server host", "127.0.0.1"),
            ("server-port", "Server port", "3307"),
            (
                "server-user",
                "Server user (password via BEADS_DOLT_PASSWORD)",
                "root",
            ),
        ] {
            loop {
                let value = answer!(ask(input, output, label, default));
                if flag == "server-port" && !valid_port(&value) {
                    writeln!(output, "Enter a port from 1 to 65535.")?;
                    continue;
                }
                args.push(format!("--{flag}={value}"));
                break;
            }
        }
    }
    let stealth = answer!(choose(
        input,
        output,
        "Stealth (keep Beads files local): yes / no",
        "no",
        &["yes", "no"]
    ));
    if stealth == "yes" {
        args.push("--stealth".into());
    }
    let hooks = answer!(choose(
        input,
        output,
        "Install Beads git hooks: yes / no",
        "no",
        &["yes", "no"]
    ));
    if hooks == "no" {
        args.push("--skip-hooks".into());
    }
    let agents = answer!(choose(
        input,
        output,
        "Generate agent instructions and tool setup: yes / no",
        "no",
        &["yes", "no"]
    ));
    if agents == "no" {
        args.push("--skip-agents".into());
    } else {
        let profile = answer!(choose(
            input,
            output,
            "Agent instructions profile: minimal / full",
            "minimal",
            &["minimal", "full"]
        ));
        args.push(format!("--agents-profile={profile}"));
    }
    let export = answer!(choose(
        input,
        output,
        "JSONL auto-export (off recommended to keep OnTrack worktrees clean): yes / no",
        "no",
        &["yes", "no"]
    )) == "yes";
    writeln!(
        output,
        "Additional bd init options: type help to list, an option name to set, or Enter to continue. Values are literal text, not shell commands."
    )?;
    loop {
        let name = answer!(ask(input, output, "Additional option", ""));
        if name.is_empty() {
            break;
        }
        if name == "help" {
            for option in options {
                writeln!(
                    output,
                    "--{} {}  {}",
                    option.name,
                    option.kind.as_deref().unwrap_or(""),
                    option.help
                )?;
            }
            continue;
        }
        let Some(option) = options
            .iter()
            .find(|option| option.name == name.trim_start_matches("--"))
        else {
            writeln!(
                output,
                "Unknown or unsafe option. Type help for available options."
            )?;
            continue;
        };
        let value = if let Some(kind) = &option.kind {
            let value = answer!(ask(input, output, &format!("{} ({kind})", option.name), ""));
            if (kind == "int" && value.parse::<u16>().is_err())
                || (option.name.ends_with("port") && !valid_port(&value))
            {
                writeln!(output, "Invalid numeric value.")?;
                continue;
            }
            format!("--{}={value}", option.name)
        } else {
            format!("--{}", option.name)
        };
        // Last selection wins, including overriding a common-field default.
        args.retain(|arg| {
            arg != &format!("--{}", option.name) && !arg.starts_with(&format!("--{}=", option.name))
        });
        args.push(value);
    }
    // We already collected the ordinary choices. Only explicit workflow wizards
    // need native prompts; otherwise avoid asking about auto-export a second time.
    if !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--contributor" | "--team"))
    {
        args.push("--non-interactive".into());
    }
    writeln!(
        output,
        "\nInitialize in this directory with arguments: {args:?}\nThen set export.auto={export}. Explicit contributor/team wizards may ask additional questions.\nAgent generation and hooks modify project files only when selected."
    )?;
    if answer!(choose(
        input,
        output,
        "Initialize and open OnTrack: yes / no",
        "no",
        &["yes", "no"]
    )) != "yes"
    {
        return Ok(None);
    }
    Ok(Some(Plan { args, export }))
}

fn valid_port(value: &str) -> bool {
    value.parse::<u16>().is_ok_and(|port| port > 0)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn response(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }
    const MISSING: &str = r#"{"error":"no_beads_directory"}"#;
    const HELP: &str = "Flags:\n  -p, --prefix string   Issue prefix\n      --remote string   Remote URL\n      --contributor     Contributor wizard\n      --server-port int   Port\n      --force           Reinit\n      --destroy-token string   Dangerous\n      --from-jsonl      Import\nGlobal Flags:\n      --db string       Override DB\n";
    // Prefix, role, storage, stealth, hooks, agents, export, extra, confirm.
    const DEFAULTS: &str = "\n\n\n\n\n\n\n\nyes\n";

    #[test]
    fn defaults_and_explicit_confirmation() {
        let mut transcript = Vec::new();
        let plan = dialogue(
            &mut DEFAULTS.as_bytes(),
            &mut transcript,
            &init_options(HELP),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            plan.args,
            strings(&[
                "--no-color",
                "init",
                "--role=maintainer",
                "--skip-hooks",
                "--skip-agents",
                "--non-interactive"
            ])
        );
        assert!(!plan.export);
        assert!(
            String::from_utf8(transcript)
                .unwrap()
                .contains("export.auto=false")
        );
        // All defaults also default the final confirmation to no.
        assert!(
            dialogue(&mut "\n".repeat(9).as_bytes(), &mut Vec::new(), &[])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn explicit_native_wizard_keeps_interactive_mode() {
        let answers = "\n\n\n\n\n\n\ncontributor\n\nyes\n";
        let plan = dialogue(
            &mut answers.as_bytes(),
            &mut Vec::new(),
            &init_options(HELP),
        )
        .unwrap()
        .unwrap();
        assert!(plan.args.contains(&"--contributor".into()));
        assert!(!plan.args.contains(&"--non-interactive".into()));
    }

    #[test]
    fn malformed_successful_discovery_is_an_error() {
        for stdout in ["", "bad json", "{}", r#"{"path":" "}"#] {
            assert_eq!(
                workspace_exists(&response(0, stdout, ""))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn custom_values_remain_literal_and_options_are_filtered() {
        let options = init_options(HELP);
        assert_eq!(
            options.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            ["prefix", "remote", "contributor", "server-port"]
        );
        let answers = "$(touch /tmp/nope)\ncontributor\nserver\nexample.org\n0\n5432\nroot\nyes\nyes\nyes\nfull\nyes\nhelp\nforce\nremote\nhttps://example.org/path with spaces\n\nyes\n";
        let mut transcript = Vec::new();
        let plan = dialogue(&mut answers.as_bytes(), &mut transcript, &options)
            .unwrap()
            .unwrap();
        assert!(plan.args.contains(&"--prefix=$(touch /tmp/nope)".into()));
        assert!(
            plan.args
                .contains(&"--remote=https://example.org/path with spaces".into())
        );
        assert!(plan.args.contains(&"--server-port=5432".into()));
        assert!(plan.args.contains(&"--agents-profile=full".into()));
        assert!(
            !plan
                .args
                .iter()
                .any(|a| a.starts_with("--skip-") || a == "--force")
        );
        assert!(plan.export);
        let transcript = String::from_utf8(transcript).unwrap();
        assert!(transcript.contains("Unknown or unsafe option"));
        assert!(transcript.contains("Enter a port"));
    }

    #[test]
    fn existing_inherited_or_redirected_workspace_has_no_writes_or_prompts() {
        for path in ["/project", "/project/nested", "/linked-worktree"] {
            let mut calls = 0;
            let mut transcript = Vec::new();
            assert!(
                setup(
                    Path::new(path),
                    false,
                    &mut io::empty(),
                    &mut transcript,
                    |args, interactive| {
                        calls += 1;
                        assert_eq!(args, &strings(&["--no-color", "where", "--json"]));
                        assert!(!interactive);
                        Ok(response(0, r#"{"path":"/project/.beads"}"#, ""))
                    }
                )
                .unwrap()
            );
            assert_eq!(calls, 1);
            assert!(transcript.is_empty());
        }
    }

    #[test]
    fn discovery_errors_do_not_trigger_setup() {
        for (stdout, stderr) in [
            ("", "permission denied"),
            ("broken json", "bad redirect"),
            (r#"{"error":"database_error"}"#, "Dolt unavailable"),
        ] {
            let mut calls = 0;
            let error = setup(
                Path::new("/project"),
                true,
                &mut DEFAULTS.as_bytes(),
                &mut Vec::new(),
                |_, _| {
                    calls += 1;
                    Ok(response(1, stdout, stderr))
                },
            )
            .unwrap_err();
            assert_eq!(calls, 1);
            assert!(error.to_string().contains(stderr));
        }
    }

    #[test]
    fn no_terminal_cancel_and_eof_never_initialize() {
        for (interactive, answers) in [
            (false, DEFAULTS),
            (true, ":cancel\n"),
            (true, ""),
            (true, "\n\n\n\n\n\n\n\nno\n"),
        ] {
            let mut calls = Vec::new();
            let result = setup(
                Path::new("/project"),
                interactive,
                &mut answers.as_bytes(),
                &mut Vec::new(),
                |args, _| {
                    calls.push(args.to_vec());
                    Ok(if calls.len() == 1 {
                        response(1, MISSING, "")
                    } else {
                        response(0, HELP, "")
                    })
                },
            );
            if interactive {
                assert!(!result.unwrap());
            } else {
                assert!(result.unwrap_err().to_string().contains("terminal"));
            }
            assert!(
                calls
                    .iter()
                    .all(|a| a.contains(&"where".into()) || a.contains(&"--help".into()))
            );
        }
    }

    #[test]
    fn setup_order_and_failure_boundaries() {
        for failure in [None, Some(0), Some(1), Some(2)] {
            let mut step = 0;
            let mut calls = Vec::new();
            let result = setup(
                Path::new("/project"),
                true,
                &mut DEFAULTS.as_bytes(),
                &mut Vec::new(),
                |args, native| {
                    calls.push((args.to_vec(), native));
                    if calls.len() == 1 {
                        return Ok(response(1, MISSING, ""));
                    }
                    let current = step;
                    step += 1;
                    if failure == Some(current) {
                        return Ok(response(1, "", "test failure"));
                    }
                    Ok(response(0, if current == 0 { HELP } else { "" }, ""))
                },
            );
            if failure.is_some() {
                assert!(result.unwrap_err().to_string().contains("test failure"));
            } else {
                assert!(result.unwrap());
            }
            assert_eq!(calls.len(), failure.map_or(4, |step| step + 2));
            if calls.len() >= 3 {
                assert!(calls[2].1);
                assert_eq!(calls[2].0[1], "init");
            }
            if failure.is_none() {
                assert_eq!(
                    calls[3].0,
                    strings(&["--no-color", "config", "set", "export.auto", "false"])
                );
            }
        }
    }

    #[test]
    fn missing_executable_and_invalid_responses_fail_safely() {
        let error = setup(
            Path::new("/project"),
            true,
            &mut io::empty(),
            &mut Vec::new(),
            |_, _| Err(io::Error::new(io::ErrorKind::NotFound, "bd missing")),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let mut transcript = Vec::new();
        let answer = choose(
            &mut "maybe\nyes\n".as_bytes(),
            &mut transcript,
            "Confirm",
            "no",
            &["yes", "no"],
        )
        .unwrap();
        assert_eq!(answer.as_deref(), Some("yes"));
        assert!(
            String::from_utf8(transcript)
                .unwrap()
                .contains("Choose yes / no")
        );
    }
}
