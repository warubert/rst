use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum CliParse {
    Help,
    Command(CliCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Extract {
        input: String,
        subtitle_language: String,
        output: String,
    },
    Translate {
        input: String,
        source_language: String,
        target_language: String,
        output: String,
    },
    ExtractAndTranslate {
        input: String,
        subtitle_language: String,
        target_language: String,
        output: String,
        translated_output: String,
    },
    Transcribe {
        input: String,
        language: String,
        output: String,
    },
    TranscribeAndTranslate {
        input: String,
        language: String,
        target_language: String,
        output: String,
        translated_output: String,
    },
}

pub fn parse_args(args: &[String]) -> Result<CliParse, String> {
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        return Ok(CliParse::Help);
    }

    let mut options = HashMap::new();
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        let Some(name) = argument.strip_prefix("--") else {
            return Err(format!("Unexpected positional argument: {argument}"));
        };
        if !matches!(
            name,
            "action"
                | "input"
                | "subtitle-language"
                | "source-language"
                | "target-language"
                | "language"
                | "output"
                | "translated-output"
        ) {
            return Err(format!("Unknown option: {argument}"));
        }
        let value = arguments
            .next()
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("Missing value for {argument}"))?;
        if options.insert(name, value.clone()).is_some() {
            return Err(format!("Option {argument} was provided more than once"));
        }
    }

    let action = required(&mut options, "action")?;
    let command = match action.as_str() {
        "extract" => CliCommand::Extract {
            input: required(&mut options, "input")?,
            subtitle_language: required(&mut options, "subtitle-language")?,
            output: required(&mut options, "output")?,
        },
        "translate" => CliCommand::Translate {
            input: required(&mut options, "input")?,
            source_language: required(&mut options, "source-language")?,
            target_language: required(&mut options, "target-language")?,
            output: required(&mut options, "output")?,
        },
        "extract-and-translate" => {
            let output = required(&mut options, "output")?;
            let translated_output = required(&mut options, "translated-output")?;
            let (output, translated_output) = distinct_output(output, translated_output)?;
            CliCommand::ExtractAndTranslate {
                input: required(&mut options, "input")?,
                subtitle_language: required(&mut options, "subtitle-language")?,
                target_language: required(&mut options, "target-language")?,
                output,
                translated_output,
            }
        }
        "transcribe" => CliCommand::Transcribe {
            input: required(&mut options, "input")?,
            language: required(&mut options, "language")?,
            output: required(&mut options, "output")?,
        },
        "transcribe-and-translate" => {
            let output = required(&mut options, "output")?;
            let translated_output = required(&mut options, "translated-output")?;
            let (output, translated_output) = distinct_output(output, translated_output)?;
            CliCommand::TranscribeAndTranslate {
                input: required(&mut options, "input")?,
                language: required(&mut options, "language")?,
                target_language: required(&mut options, "target-language")?,
                output,
                translated_output,
            }
        }
        _ => return Err(format!("Unknown action: {action}")),
    };

    if let Some((name, _)) = options.into_iter().next() {
        return Err(format!("Option --{name} is not used by action {action}"));
    }

    Ok(CliParse::Command(command))
}

fn required(options: &mut HashMap<&str, String>, name: &'static str) -> Result<String, String> {
    options
        .remove(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required option: --{name}"))
}

fn distinct_output(output: String, translated_output: String) -> Result<(String, String), String> {
    if output == translated_output {
        return Err("--output and --translated-output must be different paths".to_string());
    }
    Ok((output, translated_output))
}
