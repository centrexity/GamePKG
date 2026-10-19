pub fn rejected_paths() -> Vec<String> {
    let mut paths: Vec<String> = [
        "",
        ".",
        "..",
        "./hello.txt",
        "../hello.txt",
        "a/../../hello.txt",
        "a/./hello.txt",
        "a/../hello.txt",
        "a//hello.txt",
        "a/",
        "/hello.txt",
        "//server/share",
        "C:hello.txt",
        "C:/hello.txt",
        "c:\\hello.txt",
        "\\hello.txt",
        "\\\\server\\share",
        "\\\\?\\C:\\hello.txt",
        "\\\\.\\NUL",
        "\\??\\C:\\hello.txt",
        "a\\../hello.txt",
        "a/..\\hello.txt",
        "hello.txt:secret",
        "hello.txt::$DATA",
        "a/.. /hello.txt",
        "a/. /hello.txt",
        "a/.../hello.txt",
        "a/hello.txt.",
        "a/hello.txt ",
        "a/hello.txt. ",
        "a/..\t/hello.txt",
        "hello\0.txt",
        "hello.txt\0/../secret",
        "hello\n.txt",
        "hello\r.txt",
        "hello\t.txt",
        "a/\u{7f}/file",
        "a/\u{85}/file",
        "a/<file>",
        "a/fi\"le",
        "a/fi|le",
        "a/fi?le",
        "a/fi*le",
        "CON",
        "nul.txt",
        "AUX.tar.gz",
        "prn",
        "a/CoN/file",
        "a/COM1/file",
        "a/LPT9.txt",
        "a/COM¹.txt",
        "a/LPT²/file",
        "a/COM³",
        "CONIN$",
        "conout$.txt",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for name in ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"] {
        for case in [name.to_owned(), name.to_lowercase()] {
            for suffix in ["", ".txt", ".tar.gz", " ", ".", " .txt", "  .txt"] {
                for prefix in ["", "assets/", "source/src/"] {
                    paths.push(format!("{prefix}{case}{suffix}"));
                }
            }
        }
    }
    for prefix in ["COM", "LPT"] {
        for digit in ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"] {
            for suffix in ["", ".rs", ".blob"] {
                paths.push(format!("a/{prefix}{digit}{suffix}"));
                paths.push(format!("{prefix}{digit}{suffix}").to_lowercase());
            }
        }
    }
    for byte in (0u8..=31).chain([127]) {
        paths.push(format!("a/hello{}world", char::from(byte)));
    }
    paths.sort();
    paths.dedup();
    paths.push("a".repeat(256));
    paths.push(vec!["a"; 33].join("/"));
    paths.push(vec!["a".repeat(32); 32].join("/"));
    paths
}

// These spellings are literal archive keys. They must never be URL-decoded,
// Unicode-normalized, expanded as shell input, or resolved against host paths.
pub fn literal_paths() -> &'static [&'static str] {
    &[
        "%2e%2e/hello.txt",
        "%252e%252e/hello.txt",
        "%2fetc%2fpasswd",
        "%5c%5cserver%5cshare",
        "..%2fhello.txt",
        "..%5chello.txt",
        "．．/hello.txt",
        "․․/hello.txt",
        "a∕hello.txt",
        "a／hello.txt",
        "a＼hello.txt",
        "~/hello.txt",
        "$HOME/hello.txt",
        "${HOME}/hello.txt",
        "a/HELLO.TXT",
        "a/hello%00.txt",
        "a/hello.txt%20",
        "a/hello.txt%2e",
    ]
}
