fn main() {
    let fixtures = [
        include_str!("../../../README.md"),
        "# 😀 **bold**\n\n> *quoted*\n>\n> ```rust\n> let café = 1;\n> ```\n\n| a | b |\n| - | - |\n| | 😀 |\n",
        "```diff file=test.rs\n-old\n+new\n```\n\n![img](attachment.png)\n\n[link](https://example.com)\n\nMath $x^2$.\n",
        "", "text\n\n---\n\nlast",
    ];
    for fixture in fixtures {
        println!("{}", markit::render_document_json(fixture));
        println!(
            "{}",
            serde_json::to_string(&markit::render_html(fixture)).unwrap()
        );
    }
}
