pub fn preprocess_diff_for_ai(raw_diff: &str) -> String {
    let mut processed_lines = Vec::new();
    for line in raw_diff.lines() {
        if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("diff --git")
            || line.starts_with("index")
            || line.starts_with("old mode")
            || line.starts_with("new mode")
            || line.starts_with("deleted file mode")
            || line.starts_with("new file mode")
            || line.starts_with("copy from")
            || line.starts_with("copy to")
            || line.starts_with("rename from")
            || line.starts_with("rename to")
            || line.starts_with("similarity index")
            || line.starts_with("dissimilarity index")
            || line.starts_with("Binary files")
            || line.starts_with("@@")
        {
            processed_lines.push(line.to_string());
        } else if line.starts_with('+') {
            processed_lines.push(format!("[ADDED_LINE]: {}", &line[1..]));
        } else if line.starts_with('-') {
            processed_lines.push(format!("[REMOVED_LINE]: {}", &line[1..]));
        } else {
            processed_lines.push(line.to_string());
        }
    }
    processed_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_simple_addition() {
        let raw_diff = "diff --git a/file.txt b/file.txt\n\
                        index 000..111 100644\n\
                        --- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -0,0 +1 @@\n\
                        +new line content";
        let expected = "diff --git a/file.txt b/file.txt\n\
                        index 000..111 100644\n\
                        --- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -0,0 +1 @@\n\
                        [ADDED_LINE]: new line content";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_simple_removal() {
        let raw_diff = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +0,0 @@\n\
                        -old line content";
        let expected = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +0,0 @@\n\
                        [REMOVED_LINE]: old line content";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_modification_with_context() {
        let raw_diff = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1,3 +1,3 @@\n\
                         context before\n\
                        -old line\n\
                        +new line\n\
                         context after";
        let expected = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1,3 +1,3 @@\n\
                         context before\n\
                        [REMOVED_LINE]: old line\n\
                        [ADDED_LINE]: new line\n\
                         context after";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_no_content_changes_mode_only() {
        let raw_diff = "diff --git a/file.txt b/file.txt\n\
                        old mode 100644\n\
                        new mode 100755";
        let expected = "diff --git a/file.txt b/file.txt\n\
                        old mode 100644\n\
                        new mode 100755";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_rename_summary_lines() {
        let raw_diff = "diff --git a/old_name.txt b/new_name.txt\n\
                        similarity index 90%\n\
                        rename from old_name.txt\n\
                        rename to new_name.txt\n\
                        index 000..111 100644\n\
                        --- a/old_name.txt\n\
                        +++ b/new_name.txt\n\
                        @@ -1 +1 @@\n\
                        -old content\n\
                        +new content";
        let expected = "diff --git a/old_name.txt b/new_name.txt\n\
                        similarity index 90%\n\
                        rename from old_name.txt\n\
                        rename to new_name.txt\n\
                        index 000..111 100644\n\
                        --- a/old_name.txt\n\
                        +++ b/new_name.txt\n\
                        @@ -1 +1 @@\n\
                        [REMOVED_LINE]: old content\n\
                        [ADDED_LINE]: new content";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_binary_files_differ() {
        let raw_diff = "diff --git a/image.png b/image.png\n\
                        Binary files a/image.png and b/image.png differ";
        let expected = "diff --git a/image.png b/image.png\n\
                        Binary files a/image.png and b/image.png differ";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_preprocess_empty_input() {
        assert_eq!(preprocess_diff_for_ai(""), "");
    }

    #[test]
    fn test_preprocess_multiple_hunks() {
        let raw_diff = "diff --git a/file.txt b/file.txt\n\
                        --- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1,3 +1,3 @@\n\
                         context1\n\
                        -first old\n\
                        +first new\n\
                         context2\n\
                        @@ -10,3 +10,3 @@\n\
                         context3\n\
                        -second old\n\
                        +second new\n\
                         context4";
        let expected = "diff --git a/file.txt b/file.txt\n\
                        --- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1,3 +1,3 @@\n\
                         context1\n\
                        [REMOVED_LINE]: first old\n\
                        [ADDED_LINE]: first new\n\
                         context2\n\
                        @@ -10,3 +10,3 @@\n\
                         context3\n\
                        [REMOVED_LINE]: second old\n\
                        [ADDED_LINE]: second new\n\
                         context4";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_line_starting_with_space_after_plus_minus() {
        let raw_diff = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +1 @@\n\
                        - old line with leading space\n\
                        + new line with leading space";
        let expected = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +1 @@\n\
                        [REMOVED_LINE]:  old line with leading space\n\
                        [ADDED_LINE]:  new line with leading space";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }

    #[test]
    fn test_plus_minus_not_at_start_of_line_content() {
        let raw_diff = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +1 @@\n\
                        +this line has a + plus and a - minus sign.";
        let expected = "--- a/file.txt\n\
                        +++ b/file.txt\n\
                        @@ -1 +1 @@\n\
                        [ADDED_LINE]: this line has a + plus and a - minus sign.";
        assert_eq!(preprocess_diff_for_ai(raw_diff), expected);
    }
}
