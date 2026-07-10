use super::inline::escape_html;
use std::sync::LazyLock;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
const SYNTECT_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "syn-" };

pub(crate) fn highlight_code(lang: &str, code: &str) -> String {
    let syntax_set = &SYNTAX_SET;
    let syntax = syntax_for_lang(lang, syntax_set);
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, SYNTECT_CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return escape_html(code);
        }
    }
    generator.finalize()
}

fn syntax_for_lang<'a>(lang: &str, syntax_set: &'a SyntaxSet) -> &'a SyntaxReference {
    let lower = lang.to_ascii_lowercase();
    let token = match lower.as_str() {
        "bash" | "shell" => "sh",
        "c++" | "cxx" => "cpp",
        "emacs-lisp" | "elisp" => "el",
        "javascript" => "js",
        "python" => "py",
        "rust" => "rs",
        "typescript" => "ts",
        _ => lower.as_str(),
    };
    syntax_set
        .find_syntax_by_token(token)
        .or_else(|| syntax_set.find_syntax_by_extension(token))
        .or_else(|| syntax_set.find_syntax_by_name(lang))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn syntect_theme() -> &'static Theme {
    THEME_SET
        .themes
        .get("InspiredGitHub")
        .or_else(|| THEME_SET.themes.get("base16-ocean.dark"))
        .or_else(|| THEME_SET.themes.values().next())
        .expect("syntect default themes should include at least one theme")
}

pub(crate) fn syntect_css() -> String {
    let mut css =
        css_for_theme_with_class_style(syntect_theme(), SYNTECT_CLASS_STYLE).unwrap_or_default();
    css.push_str(
        ".code pre .syn-code,.code pre .syn-code span{background:transparent!important;background-color:transparent!important}",
    );
    css
}
