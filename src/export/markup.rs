//! Docs are Markdown; each exporter's target wants its own markup.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// The markup an exporter writes, which decides how Markdown prose becomes
/// text in its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Markup {
    /// Left as written, for exporters that output Markdown themselves.
    Markdown,
    Html,
    /// What the Nexus mod page and article editors take.
    Bbcode,
}

pub fn render(markdown: &str, markup: Markup) -> String {
    match markup {
        Markup::Markdown => markdown.trim().to_string(),
        Markup::Html => {
            let mut out = String::new();
            pulldown_cmark::html::push_html(&mut out, Parser::new_ext(markdown, options()));
            out.trim().to_string()
        }
        Markup::Bbcode => bbcode(markdown),
    }
}

fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES
}

/// Markdown as Nexus BBCode.
///
/// Nexus has no inline-code tag, so inline code becomes a monospace font, and
/// no heading tag, so headings become bold text at a size.
fn bbcode(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_code_block = false;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { level, .. } => {
                    let size = match level {
                        HeadingLevel::H1 => 6,
                        HeadingLevel::H2 => 5,
                        HeadingLevel::H3 => 4,
                        _ => 3,
                    };
                    out.push_str(&format!("[size={size}][b]"));
                }
                Tag::BlockQuote(_) => out.push_str("[quote]"),
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    out.push_str("[code]");
                }
                Tag::List(start) => {
                    out.push_str(if start.is_some() {
                        "[list=1]\n"
                    } else {
                        "[list]\n"
                    });
                }
                Tag::Item => out.push_str("[*]"),
                Tag::Emphasis => out.push_str("[i]"),
                Tag::Strong => out.push_str("[b]"),
                Tag::Strikethrough => out.push_str("[s]"),
                Tag::Link { dest_url, .. } => out.push_str(&format!("[url={dest_url}]")),
                Tag::Image { dest_url, .. } => out.push_str(&format!("[img]{dest_url}")),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("\n\n"),
                TagEnd::Heading(_) => out.push_str("[/b][/size]\n"),
                TagEnd::BlockQuote(_) => out.push_str("[/quote]\n"),
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    out.push_str("[/code]\n");
                }
                TagEnd::List(_) => out.push_str("[/list]\n"),
                TagEnd::Item => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                TagEnd::Emphasis => out.push_str("[/i]"),
                TagEnd::Strong => out.push_str("[/b]"),
                TagEnd::Strikethrough => out.push_str("[/s]"),
                TagEnd::Link => out.push_str("[/url]"),
                // The alt text has been written as the image's text; drop it.
                TagEnd::Image => out.push_str("[/img]"),
                _ => {}
            },
            Event::Text(text) => out.push_str(&text),
            Event::Code(code) => out.push_str(&format!("[font=Courier New]{code}[/font]")),
            Event::SoftBreak => out.push(if in_code_block { '\n' } else { ' ' }),
            Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("[line]\n"),
            Event::Html(html) | Event::InlineHtml(html) => out.push_str(&html),
            _ => {}
        }
    }
    // Paragraphs inside list items would otherwise leave blank lines in the list.
    out.replace("\n\n[/list]", "\n[/list]")
        .replace("\n\n[*]", "\n[*]")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_becomes_nexus_bbcode() {
        let md = "## Setup\n\nRead the *Prologue* first, **then** [the guide](https://x.y).\n\n\
                  - one `ini`\n- two\n\n1. first\n2. second\n\n```\nset a to 1\n```\n";
        assert_eq!(
            render(md, Markup::Bbcode),
            "[size=5][b]Setup[/b][/size]\n\
             Read the [i]Prologue[/i] first, [b]then[/b] [url=https://x.y]the guide[/url].\n\n\
             [list]\n[*]one [font=Courier New]ini[/font]\n[*]two\n[/list]\n\
             [list=1]\n[*]first\n[*]second\n[/list]\n\
             [code]set a to 1\n[/code]"
        );
    }

    #[test]
    fn html_and_markdown_targets() {
        assert_eq!(render("*a*", Markup::Html), "<p><em>a</em></p>");
        assert_eq!(render(" *a* \n", Markup::Markdown), "*a*");
    }
}
