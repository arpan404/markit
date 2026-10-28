pub fn sequences() -> Vec<Vec<String>> {
    let mut cases = vec![
        vec![
            "# History\n\n".repeat(8000),
            "```rust\n".into(),
            "/* comment\n".into(),
            "still comment */\nfn main() {\n".into(),
            "let s = r#\"a\n".into(),
            "b\"#;\n}\n".into(),
            "```\n\nend".into(),
        ],
        [
            "[late][id]\n\nold\n\n",
            "new\n\n",
            "[id]: https://example.com \"title\"\n",
        ]
        .map(String::from)
        .to_vec(),
        [
            "before\n\n| a | b |\n",
            "| --",
            "- | --- |\n",
            "| c | d |\n\n",
            "after\n===\n",
        ]
        .map(String::from)
        .to_vec(),
        [
            "$$x",
            " + y",
            "$$\n\n",
            "- first\n",
            "\n  second\n",
            "\nnext\n",
        ]
        .map(String::from)
        .to_vec(),
    ];
    for source in [
        "<div>raw</div>\n\npara\n\n---\n",
        "```rust\nlet a = 1;\n/* outer\n/* nested */\nend */\nlet s = r##\"raw\nstr\"##;\n```\n\nend",
        "~~~python\nx = \"\"\"long\nstr\"\"\"\ny\n(1)\n~~~\n",
        "```tsx\n<Component\n property=\"value\"/>\n<!-- c\nomment -->\n```\n",
        "```rust&amp;\nhi\n```\n", "```\n\r\nhi\0there\n```\n",
        "- item\n\n  ```rust\n  fn main() {}\n  ```\n\nend",
        "$$x\n\ny$$\n\n[ref]\n\n[ref]: /path\n",
        "```rust\n\n```\n", "```rust\nvalue  \n  \n(42)\n```\n",
    ] { cases.push(source.chars().map(|c| c.to_string()).collect()); }
    cases
}
