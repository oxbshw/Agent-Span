//! Terminal styling helpers (ANSI colors, tables, progress bars) for the CLI.
//!
//! Colors degrade to empty strings when stdout is not a TTY, so piped/redirected
//! output stays clean and parseable.
#![allow(dead_code)] // a presentation toolkit: not every helper is wired yet
#![allow(clippy::uninlined_format_args)] // keep the explicit named-arg style

use std::io::{self, IsTerminal};

macro_rules! color {
    ($name:ident, $code:expr) => {
        pub fn $name() -> &'static str {
            if io::stdout().is_terminal() {
                $code
            } else {
                ""
            }
        }
    };
}

color!(pink, "\x1b[38;2;232;165;243m");
color!(pink_dim, "\x1b[38;2;180;120;200m");
color!(coral, "\x1b[38;2;255;223;196m");
color!(coral_dim, "\x1b[38;2;200;170;150m");
color!(green, "\x1b[38;2;34;197;94m");
color!(red, "\x1b[38;2;239;68;68m");
color!(yellow, "\x1b[38;2;245;158;11m");
color!(blue, "\x1b[38;2;59;130;246m");
color!(white, "\x1b[37m");
color!(gray, "\x1b[38;2;107;114;128m");
color!(dim, "\x1b[2m");
color!(bold, "\x1b[1m");
color!(reset, "\x1b[0m");

pub fn banner() {
    if !io::stdout().is_terminal() {
        return;
    }
    let (p, c, r, g) = (pink(), coral(), reset(), gray());
    println!(
        "{c}    _                 _                  {r}",
        c = c,
        r = r
    );
    println!(
        "{p}   / \\   ___ _ __ ___| |_ __ _ _ __ ___  {r}",
        p = p,
        r = r
    );
    println!(
        "{c}  / _ \\ / _ \\ '__/ _ \\ __/ _` | '_ ` _ \\ {r}",
        c = c,
        r = r
    );
    println!(
        "{p} / ___ \\  __/ | |  __/ || (_| | | | | | |{r}",
        p = p,
        r = r
    );
    println!(
        "{c}/_/   \\_\\___|_|  \\___|\\__\\__,_|_| |_| |_|{r}",
        c = c,
        r = r
    );
    println!(
        "{g}  Web Access Gateway for AI Agents  v{}{r}",
        env!("CARGO_PKG_VERSION"),
        g = g,
        r = r
    );
    println!();
}

pub fn section(title: &str) {
    if !io::stdout().is_terminal() {
        println!("\n=== {} ===", title);
        return;
    }
    let (gr, pk, rs) = (gray(), pink(), reset());
    println!(
        "\n{gr}┌──────────────────────────────────────────┐{rs}",
        gr = gr,
        rs = rs
    );
    println!(
        "{gr}│ {pk}{:^40}{gr} │{rs}",
        title,
        pk = pk,
        gr = gr,
        rs = rs
    );
    println!(
        "{gr}└──────────────────────────────────────────┘{rs}",
        gr = gr,
        rs = rs
    );
}

pub fn status_ok(msg: &str) {
    println!("{}  {}{}", green(), msg, reset());
}
pub fn status_err(msg: &str) {
    println!("{}  {}{}", red(), msg, reset());
}
pub fn status_warn(msg: &str) {
    println!("{}  {}{}", yellow(), msg, reset());
}
pub fn status_info(msg: &str) {
    println!("{}  {}{}", blue(), msg, reset());
}

pub struct ProgressBar {
    total: usize,
    current: usize,
    width: usize,
}
impl ProgressBar {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            current: 0,
            width: 30,
        }
    }
    pub fn inc(&mut self) {
        self.current += 1;
    }
    pub fn render(&self) -> String {
        if !io::stdout().is_terminal() {
            return format!("{}/{}", self.current, self.total);
        }
        let pct = (self.current as f64 / self.total.max(1) as f64 * 100.0) as usize;
        let filled = (self.current as f64 / self.total.max(1) as f64 * self.width as f64) as usize;
        let empty = self.width - filled;
        format!(
            "{g}[{p}{}{g}{}{g}] {c}{}% ({}/{}){r}",
            "█".repeat(filled),
            "░".repeat(empty),
            pct,
            self.current,
            self.total,
            g = gray(),
            p = pink(),
            c = coral(),
            r = reset()
        )
    }
}

pub fn mask_secret(s: &str) -> String {
    if s.len() <= 6 {
        "***".to_string()
    } else {
        format!("{}****", &s[..s.len().min(6)])
    }
}

pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if !io::stdout().is_terminal() {
        println!("{}", headers.join("\t"));
        for r in rows {
            println!("{}", r.join("\t"));
        }
        return;
    }
    let (gr, pk, w, rs) = (gray(), pink(), white(), reset());
    let widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            h.len().max(
                rows.iter()
                    .map(|r| r.get(i).map_or(0, |s| s.len()))
                    .max()
                    .unwrap_or(0),
            )
        })
        .collect();

    // Top border
    print!("{gr}┌", gr = gr);
    for (i, wdt) in widths.iter().enumerate() {
        print!("{}", "─".repeat(wdt + 2));
        if i < widths.len() - 1 {
            print!("┬");
        }
    }
    println!("┐{rs}", rs = rs);

    // Headers
    print!("{gr}│ ", gr = gr);
    for (i, h) in headers.iter().enumerate() {
        print!(
            "{pk}{:width$}{gr} │ ",
            h,
            width = widths[i],
            pk = pk,
            gr = gr
        );
    }
    println!("{rs}", rs = rs);

    // Divider
    print!("{gr}├", gr = gr);
    for (i, wdt) in widths.iter().enumerate() {
        print!("{}", "─".repeat(wdt + 2));
        if i < widths.len() - 1 {
            print!("┼");
        }
    }
    println!("┤{rs}", rs = rs);

    // Rows
    for row in rows {
        print!("{gr}│ ", gr = gr);
        for (i, cell) in row.iter().enumerate() {
            print!(
                "{w}{:width$}{gr} │ ",
                cell,
                width = widths[i],
                w = w,
                gr = gr
            );
        }
        println!("{rs}", rs = rs);
    }

    // Bottom border
    print!("{gr}└", gr = gr);
    for (i, wdt) in widths.iter().enumerate() {
        print!("{}", "─".repeat(wdt + 2));
        if i < widths.len() - 1 {
            print!("┴");
        }
    }
    println!("┘{rs}", rs = rs);
}
