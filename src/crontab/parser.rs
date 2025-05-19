use std::str::Chars;

use super::hash;
use super::tokens::{
    Comment, CommentKind, CronJob, IgnoredJob, JobDescription, JobSection, Token, Unknown, Variable,
};

/// Internal state for the [`Parser`].
///
/// This struct enables us to keep the simplified `Parser::parse()`
/// API, without passing too many variables around.
///
/// Otherwise, [`Parser`] would need to hold its own state, which forces
/// the user to bind an instance to a variable. This would be a major
/// inconvenience compared to the little impact this solution has on
/// code clarity.
struct ParserState {
    tokens: Vec<Token>,
    job_uid: usize,
    job_section: Option<JobSection>,
}

/// Parse crontab into usable tokens.
///
/// [`Parser`] only provides the [`parse()`](Parser::parse()) function
/// that outputs [`Token`]s.
///
/// To read the current user's crontab, you can use
/// [`Reader::read()`](super::Reader::read()).
///
/// The [`Vec<Token>`](Token) can be fed to [`Crontab`](super::Crontab)
/// for interpreting.
pub struct Parser;

impl Parser {
    /// Parse crontab into usable tokens.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cronrunner::parser::Parser;
    /// use cronrunner::tokens::{Token, CronJob};
    ///
    /// let tokens: Vec<Token> = Parser::parse("@hourly echo ':)'");
    ///
    /// assert_eq!(
    ///     tokens,
    ///     vec![Token::CronJob(CronJob {
    ///         uid: 1,
    ///         fingerprint: 6_917_582_312_284_972_245,
    ///         tag: None,
    ///         schedule: String::from("@hourly"),
    ///         command: String::from("echo ':)'"),
    ///         description: None,
    ///         section: None,
    ///     })],
    /// )
    /// ```
    ///
    /// # Errors
    ///
    /// This function does not `Err`. Worst case scenario an empty `Vec`
    /// is returned (empty crontab) or [`Unknown`] tokens are produced
    /// if a line is not something [`Parser`] understands.
    #[must_use]
    pub fn parse(crontab: &str) -> Vec<Token> {
        let mut state = ParserState {
            tokens: Vec::new(),
            job_uid: 1,
            job_section: None,
        };

        for mut line in crontab.lines() {
            line = line.trim();
            if line.is_empty() {
                continue;
            }
            let new_token = Self::make_token_from_line(line, &mut state);
            state.tokens.push(new_token);
        }

        state.tokens
    }

    fn make_token_from_line(line: &str, state: &mut ParserState) -> Token {
        if Self::is_job(line) {
            Self::make_token_from_job_line(line, state)
        } else if Self::is_variable(line) {
            Self::make_token_from_variable_line(line)
        } else if Self::is_comment(line) {
            Self::make_token_from_comment_line(line, state)
        } else {
            Self::make_token_from_unknown_line(line)
        }
    }

    fn is_job(line: &str) -> bool {
        let first_char = line.chars().next().unwrap();
        // ^([0-9]|\*|@)
        "0123456789*@".contains(first_char)
    }

    fn make_token_from_job_line(line: &str, state: &mut ParserState) -> Token {
        match Self::make_job_token(line, state) {
            Ok(job_token @ Token::CronJob { .. }) => {
                state.job_uid += 1;
                job_token
            }
            Ok(ignored_job) => ignored_job,
            _ => Self::make_unknown_token(line),
        }
    }

    fn make_job_token(line: &str, state: &ParserState) -> Result<Token, ()> {
        let (schedule, command) = Self::split_schedule_and_command(line);

        if schedule.is_empty() || command.is_empty() {
            return Err(());
        }

        let previous_token = state.tokens.last();
        let mut description = Self::get_job_description_if_any(previous_token);
        let tag = Self::extract_tag_from_job_description(&mut description);
        let section = state.job_section.clone();

        if Self::is_job_ignored(tag.as_ref()) {
            return Ok(Token::IgnoredJob(IgnoredJob {
                tag,
                schedule,
                command,
                description,
                section,
            }));
        }

        let uid = state.job_uid;
        let fingerprint = hash::djb2(format!("uid({uid}),command({command})"));

        Ok(Token::CronJob(CronJob {
            uid,
            fingerprint,
            tag,
            schedule,
            command,
            description,
            section,
        }))
    }

    /// Split schedule and command parts of a job line.
    ///
    /// This is a naive splitter that assumes a schedule consists of
    /// either one element if it is a shortcut (e.g., `@daily`), or five
    /// elements if not (e.g., `* * * * *`, `0 12 * * *`, etc.).
    ///
    /// Once the appropriate number of elements is consumed (i.e., the
    /// schedule is consumed), it considers the rest to be the command
    /// itself.
    fn split_schedule_and_command(line: &str) -> (String, String) {
        let mut chars = line.chars();

        // Extract schedule.
        let schedule = Self::extract_schedule_from_job_chars(&mut chars);

        // The rest is the command.
        let command = String::from(chars.as_str().trim());

        (schedule, command)
    }

    /// Do the schedule extraction...
    ///
    /// ...literally. `chars` are passed as a reference to an interator
    /// that belongs to [`split_schedule_and_command()`]. This iterator
    /// is consumed as we extract schedule elements. Once we're done,
    /// the iterator is left with only the command part.
    ///
    /// First, we determine how many elements we're expecting (one or
    /// five, depending on whether the first character is '@' or not).
    ///
    /// Then, we consume the characters, and every time we encounter
    /// whitespace (i.e., we go from _something_ to _whitespace_), we
    /// count one element.
    ///
    /// [`split_schedule_and_command()`]: Parser::split_schedule_and_command
    fn extract_schedule_from_job_chars(chars: &mut Chars) -> String {
        let first_char = chars
            .next()
            .expect("if line is empty, we shouldn't be parsing a schedule in the first place");

        let mut schedule = String::from(first_char);

        let target_schedule_length = if first_char == '@' { 1 } else { 5 };

        let mut nb_elements = 0;
        let mut previous_char = first_char;
        loop {
            let Some(char) = chars.next() else {
                // Early exit, should not happen if schedule is valid.
                return schedule;
            };

            if char.is_ascii_whitespace() {
                // From _something_ to _whitespace_.
                if !previous_char.is_ascii_whitespace() {
                    nb_elements += 1;
                    if nb_elements == target_schedule_length {
                        return schedule;
                    }
                    schedule.push(' ');
                }
            } else {
                schedule.push(char);
            }

            previous_char = char;
        }
    }

    /// Extract description comment from a token (if any).
    ///
    /// Description comments are comments that start with `##` and
    /// immediately precede a job. They are used in the job list menu to
    /// give human-readable descriptions to sometimes obscure commands.
    ///
    /// This is cronrunner specific, and has nothing to do with Cron
    /// itself.
    fn get_job_description_if_any(previous_token: Option<&Token>) -> Option<JobDescription> {
        if let Some(Token::Comment(Comment {
            value: description,
            kind: CommentKind::Description,
        })) = previous_token
        {
            if !description.is_empty() {
                return Some(JobDescription(description.clone()));
            }
        }
        None
    }

    /// Extract tag from a description comment (if any).
    ///
    /// A description comment can start with a tag. A tag is opened by
    /// `%{`, and closed by the first `}` encountered. If the comment
    /// does not start with `%{`, or does not contain `}`, nothing will
    /// be extracted.
    ///
    /// Tags are used as stable identification for jobs. This is very
    /// useful if cronrunner is used in scripts.
    ///
    /// This is cronrunner specific, and has nothing to do with Cron
    /// itself.
    fn extract_tag_from_job_description(
        job_description: &mut Option<JobDescription>,
    ) -> Option<String> {
        if !job_description
            .as_ref()
            .is_some_and(|desc| desc.0.starts_with("%{") && desc.0.contains('}'))
        {
            return None;
        }
        let description = job_description.take().expect("it is 'Some'");
        let (tag, description) = description.0.split_once('}').expect("it contains '}'");

        let description = description.trim_start();
        if !description.is_empty() {
            let description = JobDescription(description.to_string());
            _ = job_description.insert(description);
        }

        let tag = tag[2..].to_string(); // '%{'
        Some(tag)
    }

    /// Determine whether a job should be ignored.
    ///
    /// If a job is ignored, it will have a special [`IgnoredJob`] type,
    /// and will not appear in the job list, and will not be selectable.
    ///
    /// To be ignored, a job must have a tag named `ignore`.
    ///
    /// This is cronrunner specific, and has nothing to do with Cron
    /// itself.
    fn is_job_ignored(tag: Option<&String>) -> bool {
        tag.is_some_and(|tag| tag == "ignore")
    }

    /// Extract section comment from a token (if any).
    ///
    /// Section comments are comments that start with `###`. They apply
    /// to all jobs beneath, up until the end or until a new section
    /// starts. They are used in the job list menu to clearly separate
    /// behaviour in case there a many jobs.
    ///
    /// This is cronrunner specific, and has nothing to do with Cron
    /// itself.
    fn get_job_section_if_any(comment_token: &Token, state: &ParserState) -> Option<JobSection> {
        if let Token::Comment(Comment {
            value: section,
            kind: CommentKind::Section,
        }) = comment_token
        {
            if !section.is_empty() {
                let uid = state
                    .job_section
                    .as_ref()
                    .map_or(1, |section| section.uid + 1);
                return Some(JobSection {
                    uid,
                    title: section.clone(),
                });
            }
        }
        None
    }

    fn is_variable(line: &str) -> bool {
        if !line.contains('=') {
            return false;
        }
        let first_char = line.chars().next().unwrap();
        // ^[a-zA-Z_"']
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_\"'".contains(first_char)
    }

    fn make_token_from_variable_line(line: &str) -> Token {
        Self::make_variable_token(line)
    }

    fn make_variable_token(line: &str) -> Token {
        let (mut identifier, mut value) = Self::split_identifier_and_value(line);

        identifier = Self::trim_quotes(identifier);
        value = Self::trim_quotes(value);

        Token::Variable(Variable {
            identifier: String::from(identifier),
            value: String::from(value),
        })
    }

    fn split_identifier_and_value(line: &str) -> (&str, &str) {
        // Even quoted, variable names cannot contain an `=` sign.
        let (identifier, value) = line
            .split_once('=')
            .expect("the string contains an '=' sign");
        (identifier.trim(), value.trim())
    }

    fn trim_quotes(subject: &str) -> &str {
        if subject.starts_with('"') && subject.ends_with('"')
            || subject.starts_with('\'') && subject.ends_with('\'')
        {
            return &subject[1..subject.len() - 1];
        }
        subject
    }

    fn is_comment(line: &str) -> bool {
        line.starts_with('#')
    }

    fn make_token_from_comment_line(line: &str, state: &mut ParserState) -> Token {
        let comment = Self::make_comment_token(line);

        if let Some(section) = Self::get_job_section_if_any(&comment, state) {
            state.job_section = Some(section);
        }

        comment
    }

    fn make_comment_token(line: &str) -> Token {
        if Self::is_section_comment(line) {
            return Token::Comment(Comment {
                value: Self::clean_section_comment(line),
                kind: CommentKind::Section,
            });
        }

        if Self::is_description_comment(line) {
            return Token::Comment(Comment {
                value: Self::clean_description_comment(line),
                kind: CommentKind::Description,
            });
        }

        Token::Comment(Comment {
            value: Self::clean_regular_comment(line),
            kind: CommentKind::Regular,
        })
    }

    fn is_section_comment(line: &str) -> bool {
        line.starts_with("###")
    }

    fn clean_section_comment(line: &str) -> String {
        String::from(line[3..].trim_start())
    }

    fn is_description_comment(line: &str) -> bool {
        // If it's a section, it can't be a description.
        line.starts_with("##") && !Self::is_section_comment(line)
    }

    fn clean_description_comment(line: &str) -> String {
        String::from(line[2..].trim_start())
    }

    fn clean_regular_comment(line: &str) -> String {
        String::from(line[1..].trim_start())
    }

    fn make_token_from_unknown_line(line: &str) -> Token {
        Self::make_unknown_token(line)
    }

    fn make_unknown_token(line: &str) -> Token {
        Token::Unknown(Unknown {
            value: String::from(line),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_lines)]
    #[test]
    fn regular_crontab() {
        let tokens = Parser::parse(
            "
            # CronRunner Demo
            # ---------------

            @reboot /usr/bin/bash ~/startup.sh

            # Double-hash comments (##) immediately preceding a job are used as
            # description. See below:

            ## Update brew.
            30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade

            ### Some testing going on here...

            FOO=bar
            ## Print variable.
            * * * * * echo $FOO

            # Do nothing (this is a regular comment).
            @reboot :
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("CronRunner Demo"),
                    kind: CommentKind::Regular,
                }),
                Token::Comment(Comment {
                    value: String::from("---------------"),
                    kind: CommentKind::Regular,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 17_695_356_924_205_779_724,
                    tag: None,
                    schedule: String::from("@reboot"),
                    command: String::from("/usr/bin/bash ~/startup.sh"),
                    description: None,
                    section: None,
                }),
                Token::Comment(Comment {
                    value: String::from(
                        "Double-hash comments (##) immediately preceding a job are used as"
                    ),
                    kind: CommentKind::Regular,
                }),
                Token::Comment(Comment {
                    value: String::from("description. See below:"),
                    kind: CommentKind::Regular,
                }),
                Token::Comment(Comment {
                    value: String::from("Update brew."),
                    kind: CommentKind::Description,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 8_740_762_385_512_907_025,
                    tag: None,
                    schedule: String::from("30 20 * * *"),
                    command: String::from(
                        "/usr/local/bin/brew update && /usr/local/bin/brew upgrade"
                    ),
                    description: Some(JobDescription(String::from("Update brew."))),
                    section: None,
                }),
                Token::Comment(Comment {
                    value: String::from("Some testing going on here..."),
                    kind: CommentKind::Section,
                }),
                Token::Variable(Variable {
                    identifier: String::from("FOO"),
                    value: String::from("bar")
                }),
                Token::Comment(Comment {
                    value: String::from("Print variable."),
                    kind: CommentKind::Description,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 17_118_619_922_108_271_534,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("echo $FOO"),
                    description: Some(JobDescription(String::from("Print variable."))),
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Some testing going on here...")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::from("Do nothing (this is a regular comment)."),
                    kind: CommentKind::Regular,
                }),
                Token::CronJob(CronJob {
                    uid: 4,
                    fingerprint: 15_438_538_048_322_941_730,
                    tag: None,
                    schedule: String::from("@reboot"),
                    command: String::from(":"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Some testing going on here...")
                    }),
                })
            ]
        );
    }

    #[test]
    fn job_ids_are_unique_and_sequential() {
        let tokens = Parser::parse(
            "* * * * * printf 'hello, world'
             * * * * * printf 'hello, world'
             * * * * * printf 'hello, world'",
        );

        assert_eq!(
            tokens,
            vec![
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 6_015_366_411_386_091_056,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn fingerprints_are_unique() {
        let tokens = Parser::parse(
            "* * * * * printf 'hello, world'
             * * * * * printf 'hello, world'
             * * * * * printf 'hello, world'",
        );

        let Token::CronJob(job_0) = &tokens[0] else {
            panic!()
        };
        let Token::CronJob(job_1) = &tokens[1] else {
            panic!()
        };
        let Token::CronJob(job_2) = &tokens[2] else {
            panic!()
        };

        assert_eq!(job_0.fingerprint, 2_907_059_941_167_361_582);
        assert_eq!(job_1.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job_2.fingerprint, 6_015_366_411_386_091_056);
    }

    #[test]
    fn fingerprint_changes_if_uid_changes() {
        let tokens = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        let Token::CronJob(job) = &tokens[1] else {
            panic!()
        };

        assert_eq!(job.uid, 2);
        assert_eq!(job.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job.command, "printf 'hello, world'");

        let tokens = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'baz'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        let Token::CronJob(job) = &tokens[2] else {
            panic!()
        };

        assert_eq!(job.uid, 3);
        assert_eq!(job.fingerprint, 6_015_366_411_386_091_056);
        assert_eq!(job.command, "printf 'hello, world'");
    }

    #[test]
    fn fingerprint_is_stable_if_only_the_surroundings_change() {
        let tokens = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        let Token::CronJob(job) = &tokens[1] else {
            panic!()
        };

        assert_eq!(job.uid, 2);
        assert_eq!(job.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job.command, "printf 'hello, world'");

        let tokens = Parser::parse(
            "* * * * * printf 'bar'
             FOO=bar
             1 2 3 4 5 printf 'hello, world'
             1 2 3 4 5 printf 'baz'
             * * * * * printf 'foo'",
        );

        let Token::CronJob(job) = &tokens[2] else {
            panic!()
        };

        assert_eq!(job.uid, 2);
        assert_eq!(job.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job.command, "printf 'hello, world'");
    }

    #[test]
    fn fingerprint_changes_if_command_changes() {
        let tokens = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        let Token::CronJob(job) = &tokens[1] else {
            panic!()
        };

        assert_eq!(job.uid, 2);
        assert_eq!(job.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job.command, "printf 'hello, world'");

        let tokens = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'hello, goodbye'
             * * * * * printf 'bar'",
        );

        let Token::CronJob(job) = &tokens[1] else {
            panic!()
        };

        assert_eq!(job.uid, 2);
        assert_eq!(job.fingerprint, 6_767_435_073_018_149_136);
        assert_eq!(job.command, "printf 'hello, goodbye'");
    }

    #[test]
    fn tag_is_extracted_from_description_regular() {
        let tokens = Parser::parse(
            "
            ## %{tag} Job description
            @daily printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("%{tag} Job description"),
                    kind: CommentKind::Description
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: Some(String::from("tag")),
                    schedule: String::from("@daily"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(JobDescription(String::from("Job description"))),
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn tag_is_extracted_from_description_no_whitespace() {
        let tokens = Parser::parse(
            "
            ##%{tag}Job description
            @daily printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("%{tag}Job description"),
                    kind: CommentKind::Description
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: Some(String::from("tag")),
                    schedule: String::from("@daily"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(JobDescription(String::from("Job description"))),
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn tag_is_extracted_from_description_weird_characters() {
        let tokens = Parser::parse(
            "
            ## %{[{é&ù°àé \\3}]}Job description
            @daily printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("%{[{é&ù°àé \\3}]}Job description"),
                    kind: CommentKind::Description
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: Some(String::from("[{é&ù°àé \\3")),
                    schedule: String::from("@daily"),
                    command: String::from("printf 'hello, world'"),
                    // It's only up until the first `}`.
                    description: Some(JobDescription(String::from("]}Job description"))),
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn tag_is_extracted_from_description_leaves_description_empty() {
        let tokens = Parser::parse(
            "
            ## %{tag}
            @daily printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("%{tag}"),
                    kind: CommentKind::Description
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: Some(String::from("tag")),
                    schedule: String::from("@daily"),
                    command: String::from("printf 'hello, world'"),
                    // It's only up until the first `}`.
                    description: None,
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn ignored_jobs_have_their_own_type() {
        let tokens = Parser::parse(
            "
            ## %{ignore}
            @daily printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("%{ignore}"),
                    kind: CommentKind::Description
                }),
                Token::IgnoredJob(IgnoredJob {
                    tag: Some(String::from("ignore")),
                    schedule: String::from("@daily"),
                    command: String::from("printf 'hello, world'"),
                    // It's only up until the first `}`.
                    description: None,
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn ignored_jobs_have_no_influence() {
        let tokens_without_ignored = Parser::parse(
            "* * * * * printf 'foo'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        let tokens_with_ignored = Parser::parse(
            "* * * * * printf 'foo'
             ## %{ignore} Ignore the `baz` job.
             * * * * * printf 'baz'
             * * * * * printf 'hello, world'
             * * * * * printf 'bar'",
        );

        // Note: Can't do that, as the synax trees are still different,
        // but that's the spirit of the test.
        // assert_eq!(tokens_without_ignored, tokens_with_ignored);

        let Token::CronJob(job_without_ignored) = &tokens_without_ignored[1] else {
            panic!()
        };
        // Index `3` because, while the job is ignored, we still added
        // a `Comment` and an `IgnoredJob` to the AST.
        let Token::CronJob(job_with_ignored) = &tokens_with_ignored[3] else {
            panic!()
        };

        // Despite having been 'pushed down' the list by an ignored job,
        // the UID and fingerprint hasn't changed.
        assert_eq!(job_without_ignored.uid, 2);
        assert_eq!(job_without_ignored.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job_without_ignored.command, "printf 'hello, world'");

        assert_eq!(job_with_ignored.uid, 2);
        assert_eq!(job_with_ignored.fingerprint, 4_461_213_176_276_726_319);
        assert_eq!(job_with_ignored.command, "printf 'hello, world'");
    }

    #[test]
    fn shortcuts_are_parsed_as_full_job_schedule() {
        let tokens = Parser::parse(" \t@shortcut\techo 'foo'");

        let Token::CronJob(CronJob { ref schedule, .. }) = tokens[0] else {
            panic!("first (and only) token should be a job")
        };

        assert_eq!(schedule, "@shortcut");
    }

    #[test]
    fn complex_job_schedules_are_parsed_correctly() {
        let tokens = Parser::parse(" \t*/15 3-6,9-12 * * *\techo 'foo'");

        let Token::CronJob(CronJob { ref schedule, .. }) = tokens[0] else {
            panic!("first (and only) token should be a job")
        };

        assert_eq!(schedule, "*/15 3-6,9-12 * * *");
    }

    #[test]
    fn whitespace_is_cleared_around_job_schedule_and_normalized_within() {
        let tokens = Parser::parse(" \t  * \t 3-6,9-12 \t * \t * \t *   \t  echo  \t 'foo'  \t ");

        let Token::CronJob(CronJob { ref schedule, .. }) = tokens[0] else {
            panic!("first (and only) token should be a job")
        };

        assert_eq!(schedule, "* 3-6,9-12 * * *");
    }

    #[test]
    fn whitespace_is_cleared_around_job_command_but_preserved_within() {
        let tokens = Parser::parse(" \t  * \t * \t * \t * \t *   \t  echo  'foo \t\t\\n bar'  \t ");

        let Token::CronJob(CronJob { ref command, .. }) = tokens[0] else {
            panic!("first (and only) token should be a job")
        };

        assert_eq!(command, "echo  'foo \t\t\\n bar'");
    }

    #[test]
    fn tabs_are_treated_as_valid_job_delimiters() {
        let tokens = Parser::parse("\t*\t*\t\t*\t*\t*\t\techo\t\t'foo'\t\t");

        let Token::CronJob(CronJob {
            ref schedule,
            ref command,
            ..
        }) = tokens[0]
        else {
            panic!("first (and only) token should be a job")
        };

        assert_eq!(schedule, "* * * * *");
        assert_eq!(command, "echo\t\t'foo'");
    }

    #[test]
    fn false_positive_job_detections_are_marked_unknown() {
        let tokens = Parser::parse("  * * *  ");

        let Token::Unknown(Unknown { ref value }) = tokens[0] else {
            panic!("first (and only) token should be unknown")
        };

        assert_eq!(value, "* * *");
    }

    #[test]
    fn regular_comments_are_detected() {
        let tokens = Parser::parse("# Regular comment");

        assert_eq!(
            tokens,
            vec![Token::Comment(Comment {
                value: String::from("Regular comment"),
                kind: CommentKind::Regular,
            })]
        );
    }

    #[test]
    fn empty_regular_comments_are_cleared() {
        let tokens = Parser::parse("#   \n#");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Regular,
                }),
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Regular,
                })
            ]
        );
    }

    #[test]
    fn description_comments_are_detected() {
        let tokens = Parser::parse("## Job description");

        assert_eq!(
            tokens,
            vec![Token::Comment(Comment {
                value: String::from("Job description"),
                kind: CommentKind::Description,
            })]
        );
    }

    #[test]
    fn empty_description_comments_are_cleared() {
        let tokens = Parser::parse("##   \n##");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Description,
                }),
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Description,
                })
            ]
        );
    }

    #[test]
    fn description_comments_apply_to_jobs() {
        let tokens = Parser::parse("## Job description\n* * * * * printf 'hello, world'");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("Job description"),
                    kind: CommentKind::Description,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(JobDescription(String::from("Job description"))),
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn empty_description_comments_do_not_apply_to_jobs() {
        let tokens = Parser::parse("##\n* * * * * printf 'hello, world'");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Description,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn description_detection_does_not_fail_if_nothing_precedes_job() {
        let tokens = Parser::parse("* * * * * printf 'hello, world'");

        assert_eq!(
            tokens,
            vec![Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 2_907_059_941_167_361_582,
                tag: None,
                schedule: String::from("* * * * *"),
                command: String::from("printf 'hello, world'"),
                description: None,
                section: None,
            })]
        );
    }

    #[test]
    fn section_comments_are_detected() {
        let tokens = Parser::parse("### Job section");

        assert_eq!(
            tokens,
            vec![Token::Comment(Comment {
                value: String::from("Job section"),
                kind: CommentKind::Section,
            })]
        );
    }

    #[test]
    fn empty_section_comments_are_cleared() {
        let tokens = Parser::parse("###   \n###");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Section,
                }),
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Section,
                })
            ]
        );
    }

    #[test]
    fn section_comments_apply_to_jobs_beneath() {
        let tokens = Parser::parse(
            "
            ### Job section
            * * * * * printf 'hello, world'
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                })
            ]
        );
    }

    #[test]
    fn empty_section_comments_do_not_apply_to_jobs_beneath() {
        let tokens = Parser::parse("###\n* * * * * printf 'hello, world'");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                })
            ]
        );
    }

    #[test]
    fn section_comments_override_themselves() {
        let tokens = Parser::parse(
            "
            * * * * * printf 'hello, world'
            ### Job section 1
            ### Job section 2
            * * * * * printf 'hello, world'
            ### Job section 3
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::Comment(Comment {
                    value: String::from("Job section 1"),
                    kind: CommentKind::Section,
                }),
                Token::Comment(Comment {
                    value: String::from("Job section 2"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 2,
                        title: String::from("Job section 2")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::from("Job section 3"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 6_015_366_411_386_091_056,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 3,
                        title: String::from("Job section 3")
                    }),
                })
            ]
        );
    }

    #[test]
    fn duplicate_sections_are_kept_separate_even_if_consecutive() {
        let tokens = Parser::parse(
            "
            * * * * * printf 'hello, world'
            ### Job section
            * * * * * printf 'hello, world'
            ### Job section
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 6_015_366_411_386_091_056,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 2,
                        title: String::from("Job section")
                    }),
                })
            ]
        );

        let Token::CronJob(CronJob {
            section: section1, ..
        }) = &tokens[2]
        else {
            panic!()
        };
        let Token::CronJob(CronJob {
            section: section2, ..
        }) = &tokens[4]
        else {
            panic!()
        };

        assert_ne!(section1, section2);
    }

    #[test]
    fn duplicate_sections_are_treated_as_distinct() {
        let tokens = Parser::parse(
            "
            ### Job section A
            * * * * * printf 'hello, world'
            ### Other section B
            * * * * * printf 'hello, world'
            ### Job section A
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("Job section A"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section A")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::from("Other section B"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 2,
                        title: String::from("Other section B")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::from("Job section A"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 6_015_366_411_386_091_056,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 3,
                        title: String::from("Job section A")
                    }),
                })
            ]
        );
    }

    #[test]
    fn empty_section_comments_do_not_clear_previous_sections() {
        let tokens = Parser::parse(
            "
            * * * * * printf 'hello, world'
            ### Job section
            * * * * * printf 'hello, world'
            ###
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                }),
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    fingerprint: 6_015_366_411_386_091_056,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                })
            ]
        );
    }

    #[test]
    fn section_comments_and_job_descriptions_work_independently() {
        let tokens = Parser::parse(
            "
            ### Job section

            ## Job description
            * * * * * printf 'hello, world'
            * * * * * printf 'hello, world'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::Comment(Comment {
                    value: String::from("Job description"),
                    kind: CommentKind::Description,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 2_907_059_941_167_361_582,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(JobDescription(String::from("Job description"))),
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    fingerprint: 4_461_213_176_276_726_319,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                })
            ]
        );
    }

    #[test]
    fn section_comments_are_not_mistaken_as_descriptions() {
        let tokens = Parser::parse(
            "
            ### Job section
            * * * * * printf 'buongiorno'
            ",
        );

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("Job section"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 1,
                    fingerprint: 1_621_249_689_450_973_832,
                    tag: None,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'buongiorno'"),
                    description: None,
                    section: Some(JobSection {
                        uid: 1,
                        title: String::from("Job section")
                    }),
                }),
            ]
        );
    }

    #[test]
    fn unknown_token() {
        let tokens = Parser::parse("# The following line is unknown:\nunknown :");

        assert_eq!(
            tokens,
            vec![
                Token::Comment(Comment {
                    value: String::from("The following line is unknown:"),
                    kind: CommentKind::Regular,
                }),
                Token::Unknown(Unknown {
                    value: String::from("unknown :")
                }),
            ],
        );
    }

    #[test]
    fn whitespace_is_cleared_around_variables() {
        let tokens = Parser::parse("   FOO     =   bar   ");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_with_value_containing_equal_sign() {
        let tokens = Parser::parse("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("DBUS_SESSION_BUS_ADDRESS"),
                value: String::from("unix:path=/run/user/1000/bus")
            })],
        );
    }

    #[test]
    fn variable_identifier_with_single_quotes() {
        let tokens = Parser::parse("'FOO'=bar");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_identifier_with_double_quotes() {
        let tokens = Parser::parse("\"FOO\"=bar");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_value_with_single_quotes() {
        let tokens = Parser::parse("FOO='bar'");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_value_with_double_quotes() {
        let tokens = Parser::parse("FOO=\"bar\"");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_identifier_and_value_with_double_quotes() {
        let tokens = Parser::parse("\"FOO\"=\"bar\"");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_identifier_and_value_with_single_quotes() {
        let tokens = Parser::parse("'FOO'='bar'");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_identifier_with_quoted_double_quotes() {
        let tokens = Parser::parse("'\"FOO\"'=bar");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("\"FOO\""),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_identifier_with_quoted_single_quotes() {
        let tokens = Parser::parse("\"'FOO'\"=bar");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("'FOO'"),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_value_with_quoted_double_quotes() {
        let tokens = Parser::parse("FOO='\"bar\"'");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("\"bar\"")
            })],
        );
    }

    #[test]
    fn variable_value_with_quoted_single_quotes() {
        let tokens = Parser::parse("FOO=\"'bar'\"");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("'bar'")
            })],
        );
    }

    #[test]
    fn variable_quoted_identifier_with_spaces() {
        let tokens = Parser::parse("'   FOO   BAZ   '=bar");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("   FOO   BAZ   "),
                value: String::from("bar")
            })],
        );
    }

    #[test]
    fn variable_unquoted_value_with_hash() {
        let tokens = Parser::parse("FOO=bar # baz");

        assert_eq!(
            tokens,
            vec![Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar # baz")
            })],
        );
    }
}
