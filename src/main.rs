use git2::Repository;
use serde::Deserialize;
use std::io::{self, Read};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
struct Input {
    workspace: Workspace,
    model: Option<Model>,
    effort: Option<Effort>,
    cost: Option<Cost>,
    context_window: Option<ContextWindow>,
    rate_limits: Option<RateLimits>,
}

#[derive(Deserialize)]
struct Workspace {
    current_dir: String,
}

#[derive(Deserialize)]
struct Model {
    display_name: String,
}

#[derive(Deserialize)]
struct Effort {
    level: String,
}

#[derive(Deserialize)]
struct Cost {
    total_cost_usd: f64,
}

#[derive(Deserialize)]
struct ContextWindow {
    total_input_tokens: u64,
    total_output_tokens: u64,
    used_percentage: Option<f64>,
}

#[derive(Deserialize)]
struct RateLimits {
    seven_day: Option<RateWindow>,
}

#[derive(Deserialize)]
struct RateWindow {
    used_percentage: Option<f64>,
    resets_at: Option<f64>,
}

// Monokai Pro palette at ~60% brightness
const GREEN: &str = "\x1b[38;2;122;158;86m";
const CYAN: &str = "\x1b[38;2;90;158;160m";
const PURPLE: &str = "\x1b[38;2;122;109;176m";
const YELLOW: &str = "\x1b[38;2;176;154;66m";
const LIGHT_GRAY: &str = "\x1b[37m";
const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";

fn git_info(path: &str) -> Option<String> {
    let repo = Repository::open(path).ok()?;
    let head = repo.head().ok()?;
    let branch = head.shorthand()?.to_string();

    let dirty = repo
        .statuses(Some(
            git2::StatusOptions::new()
                .include_untracked(true)
                .exclude_submodules(true),
        ))
        .ok()
        .map_or(false, |s| !s.is_empty());

    let dirty_marker = if dirty { "*" } else { "" };
    Some(format!("{PURPLE}{}{dirty_marker}{RESET}", branch))
}

fn human_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{}k", n / 1000)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        format!("{}", n)
    }
}

fn human_duration(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{m}m")
    }
}

fn short_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    let components: Vec<&str> = path.rsplitn(3, '/').collect();
    match components.len() {
        0 => path.to_string(),
        1 => components[0].to_string(),
        2 => {
            if components[1].is_empty() {
                components[0].to_string()
            } else {
                format!("{}/{}", components[1], components[0])
            }
        }
        _ => {
            format!("{}/{}", components[1], components[0])
        }
    }
}

fn main() {
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return;
    }
    let input: Input = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(_) => return,
    };

    let sep = format!(" {GRAY}|{RESET} ");
    let mut segments: Vec<String> = Vec::new();

    // Model name + effort level
    if let Some(ref model) = input.model {
        let effort_suffix = input
            .effort
            .as_ref()
            .map(|e| format!(" {GRAY}({RESET}{YELLOW}{}{RESET}{GRAY}){RESET}", e.level))
            .unwrap_or_default();
        segments.push(format!(
            "{GREEN}{}{RESET}{effort_suffix}",
            model.display_name
        ));
    }

    // Short path
    segments.push(format!(
        "{CYAN}{}{RESET}",
        short_path(&input.workspace.current_dir)
    ));

    // Git branch + dirty
    if let Some(git) = git_info(&input.workspace.current_dir) {
        segments.push(git);
    }

    // Context window usage
    if let Some(ref ctx) = input.context_window {
        if let Some(pct) = ctx.used_percentage {
            segments.push(format!("{YELLOW}{:.0}% ctx{RESET}", pct));
        }
    }

    // Token count + cost (combined)
    let total_tokens = input
        .context_window
        .as_ref()
        .map(|ctx| ctx.total_input_tokens + ctx.total_output_tokens);
    match (total_tokens, input.cost.as_ref()) {
        (Some(tks), Some(cost)) => {
            segments.push(format!(
                "{LIGHT_GRAY}{}/${:.2} tks{RESET}",
                human_tokens(tks),
                cost.total_cost_usd
            ));
        }
        (Some(tks), None) => {
            segments.push(format!("{LIGHT_GRAY}{} tks{RESET}", human_tokens(tks)));
        }
        (None, Some(cost)) => {
            segments.push(format!("{LIGHT_GRAY}${:.2}{RESET}", cost.total_cost_usd));
        }
        _ => {}
    }

    // Weekly rate limit usage + reset countdown
    if let Some(week) = input
        .rate_limits
        .as_ref()
        .and_then(|r| r.seven_day.as_ref())
    {
        if let (Some(pct), Some(resets_at)) = (week.used_percentage, week.resets_at) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let left = resets_at as i64 - now as i64;
            if left > 0 {
                segments.push(format!(
                    "{CYAN}{:.0}%/7d ↻{}{RESET}",
                    pct,
                    human_duration(left as u64)
                ));
            }
        }
    }

    print!("{}", segments.join(&sep));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_tokens_small() {
        assert_eq!(human_tokens(847), "847");
    }

    #[test]
    fn test_human_tokens_low_k() {
        assert_eq!(human_tokens(1234), "1.2k");
    }

    #[test]
    fn test_human_tokens_mid_k() {
        assert_eq!(human_tokens(42000), "42k");
    }

    #[test]
    fn test_human_tokens_millions() {
        assert_eq!(human_tokens(1_523_400), "1.5M");
    }

    #[test]
    fn test_human_tokens_zero() {
        assert_eq!(human_tokens(0), "0");
    }

    #[test]
    fn test_human_duration_days() {
        assert_eq!(human_duration(2 * 86_400 + 3 * 3600 + 59 * 60), "2d3h");
    }

    #[test]
    fn test_human_duration_hours() {
        assert_eq!(human_duration(5 * 3600 + 12 * 60 + 30), "5h12m");
    }

    #[test]
    fn test_human_duration_minutes() {
        assert_eq!(human_duration(42 * 60 + 5), "42m");
    }

    #[test]
    fn test_short_path_two_components() {
        assert_eq!(
            short_path("/Users/selkie/src/github.com/tinnet/ccline"),
            "tinnet/ccline"
        );
    }

    #[test]
    fn test_short_path_one_component() {
        assert_eq!(short_path("/tmp"), "tmp");
    }

    #[test]
    fn test_short_path_root() {
        assert_eq!(short_path("/"), "/");
    }
}
