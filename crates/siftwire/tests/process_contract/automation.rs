use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};
use tempfile::TempDir;

use super::{BINARY, invoke};

const LAUNCHER: &str = include_str!("../../automation/run-siftwire-automation.sh");
const PROMPT: &str = include_str!("../../automation/siftwire-render.md");

#[test]
fn automation_replaces_itself_with_the_embedded_launcher() -> Result<()> {
    let temp = TempDir::new()?;
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    let capture = temp.path().join("capture");
    fs::create_dir_all(&bin)?;
    fs::create_dir_all(&home)?;
    let bash = bin.join("bash");
    fs::write(
        &bash,
        format!(
            "#!/bin/sh\n{{ printf '%s\\n' \"$SIFTWIRE_BIN\" \"$SIFTWIRE_RENDER_PROMPT\"; printf '%s\\0' \"$@\"; }} >'{}'\nexit 7\n",
            capture.display()
        ),
    )?;
    fs::set_permissions(&bash, fs::Permissions::from_mode(0o755))?;
    let home_text = home.to_string_lossy().into_owned();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let mut environment = BTreeMap::from([("HOME", home_text.as_str()), ("PATH", path.as_str())]);

    let output = invoke(&["automation", "--replay", "a", "b"], None, &environment)?;
    assert_eq!(output.status.code(), Some(7), "launcher exit: {output:?}");
    let captured = fs::read_to_string(&capture)?;
    let (variables, arguments) = captured.split_once("\n-c\0").context("bash arguments")?;
    let prompt = home
        .join(".local/share/siftwire/automation")
        .join(env!("CARGO_PKG_VERSION"))
        .join("siftwire-render.md");
    assert_eq!(
        variables,
        format!("{BINARY}\n{}", prompt.display()),
        "launcher environment"
    );
    assert_eq!(
        arguments,
        format!("{LAUNCHER}\0siftwire-automation\0--replay\0a\0b\0"),
        "launcher arguments"
    );
    assert_eq!(fs::read_to_string(&prompt)?, PROMPT, "installed prompt");
    assert_eq!(fs::metadata(&prompt)?.permissions().mode() & 0o777, 0o600);
    let directory = prompt.parent().context("prompt directory")?;
    assert_eq!(fs::metadata(directory)?.permissions().mode() & 0o777, 0o700);

    fs::remove_dir_all(directory)?;
    environment.insert("SIFTWIRE_BIN", "custom-binary");
    environment.insert("SIFTWIRE_RENDER_PROMPT", "custom-prompt");
    let output = invoke(&["automation"], None, &environment)?;
    assert_eq!(output.status.code(), Some(7), "override exit: {output:?}");
    assert!(
        fs::read_to_string(&capture)?
            .starts_with(&format!("custom-binary\n{}\n", prompt.display())),
        "the binary override reaches the launcher; the embedded prompt always does"
    );
    assert_eq!(fs::read_to_string(&prompt)?, PROMPT, "reinstalled prompt");
    Ok(())
}
