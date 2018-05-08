use std::process::Command;

pub fn run(command: &str) -> Result<(String, String), ::std::io::Error> {
    println!("=> {}", command);

    let mut args: Vec<_> = command.split_whitespace().collect();

    if args.is_empty() {
        // doing nothing...
        return Ok((String::new(), String::new()));
    }

    let output = Command::new(args.remove(0))
        .args(args)
        .output()?;

    Ok((
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string()
    ))
}
