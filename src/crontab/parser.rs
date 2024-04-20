// cronrunner — Run cron jobs manually.
// Copyright (C) 2024  Quentin Richert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use super::tokens::{Comment, CommentKind, CronJob, Token, Unknown, Variable};
use std::str::Chars;

/// Parse `crontab` into usable tokens.
///
/// [`Parser`] only provides the [`parse()`](Parser::parse()) function
/// that outputs [`Token`]s.
///
/// To read the current user's `crontab`, you can use
/// [`Reader::read()`](super::Reader::read()).
///
/// The [`Vec<Token>`](Token) can be fed to [`Crontab`](super::Crontab)
/// for interpreting.
pub struct Parser;

impl Parser {
    /// Parse `crontab` into usable tokens.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cronrunner::crontab::{Parser, Token, CronJob};
    ///
    /// let tokens: Vec<Token> = Parser::parse("@hourly echo ':)'");
    ///
    /// assert_eq!(
    ///     tokens,
    ///     vec![Token::CronJob(CronJob {
    ///         uid: 1,
    ///         schedule: String::from("@hourly"),
    ///         command: String::from("echo ':)'"),
    ///         description: None,section: None,
    ///     })],
    /// )
    /// ```
    ///
    /// # Errors
    ///
    /// This function does not `Err`. Worst case scenario an empty `Vec`
    /// is returned (empty `crontab`) or [`Unknown`] tokens are produced
    /// if a line is not something [`Parser`] understands.
    #[must_use]
    pub fn parse(crontab: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut job_uid: u32 = 1;
        let mut job_section: Option<String> = None;

        // TODO(refactor): This is getting unwieldy, separate into
        //  lexing and parsing, even at the cost of doing two passes.
        for mut line in crontab.lines() {
            line = line.trim();
            if line.is_empty() {
                continue;
            }
            if Self::is_job(line) {
                if let Ok(job_token) =
                    Self::make_job_token(line, tokens.last(), job_uid, &job_section)
                {
                    job_uid += 1;
                    tokens.push(job_token);
                } else {
                    tokens.push(Self::make_unknown_token(line));
                }
            } else if Self::is_variable(line) {
                tokens.push(Self::make_variable_token(line));
            } else if Self::is_comment(line) {
                let comment = Self::make_comment_token(line);

                if let Some(section) = Self::get_job_section(&comment) {
                    job_section = Some(section);
                }

                tokens.push(comment);
            } else {
                tokens.push(Self::make_unknown_token(line));
            }
        }

        tokens
    }

    fn is_job(line: &str) -> bool {
        let first_char = line.chars().next().unwrap();
        // ^([0-9]|\*|@)
        "0123456789*@".contains(first_char)
    }

    fn make_job_token(
        line: &str,
        previous_token: Option<&Token>,
        job_uid: u32,
        job_section: &Option<String>,
    ) -> Result<Token, ()> {
        let (schedule, command) = Self::split_schedule_and_command(line);

        if schedule.is_empty() || command.is_empty() {
            return Err(());
        }

        let description = Self::get_job_description(previous_token);

        Ok(Token::CronJob(CronJob {
            uid: job_uid,
            schedule,
            command,
            description,
            section: job_section.clone(),
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
    fn get_job_description(previous_token: Option<&Token>) -> Option<String> {
        if let Some(Token::Comment(Comment {
            value: description,
            kind: CommentKind::Description,
        })) = previous_token
        {
            if !description.is_empty() {
                return Some(description.clone());
            }
        }
        None
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
    fn get_job_section(comment_token: &Token) -> Option<String> {
        if let Token::Comment(Comment {
            value: section,
            kind: CommentKind::Section,
        }) = comment_token
        {
            if !section.is_empty() {
                return Some(section.clone());
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

    fn make_unknown_token(line: &str) -> Token {
        Token::Unknown(Unknown {
            value: String::from(line),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                    schedule: String::from("30 20 * * *"),
                    command: String::from(
                        "/usr/local/bin/brew update && /usr/local/bin/brew upgrade"
                    ),
                    description: Some(String::from("Update brew.")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("echo $FOO"),
                    description: Some(String::from("Print variable.")),
                    section: Some(String::from("Some testing going on here...")),
                }),
                Token::Comment(Comment {
                    value: String::from("Do nothing (this is a regular comment)."),
                    kind: CommentKind::Regular,
                }),
                Token::CronJob(CronJob {
                    uid: 4,
                    schedule: String::from("@reboot"),
                    command: String::from(":"),
                    description: None,
                    section: Some(String::from("Some testing going on here...")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                })
            ]
        );
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(String::from("Job description")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: None,
                })
            ]
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section")),
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section 2")),
                }),
                Token::Comment(Comment {
                    value: String::from("Job section 3"),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section 3")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section")),
                }),
                Token::Comment(Comment {
                    value: String::new(),
                    kind: CommentKind::Section,
                }),
                Token::CronJob(CronJob {
                    uid: 3,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: Some(String::from("Job description")),
                    section: Some(String::from("Job section")),
                }),
                Token::CronJob(CronJob {
                    uid: 2,
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'hello, world'"),
                    description: None,
                    section: Some(String::from("Job section")),
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
                    schedule: String::from("* * * * *"),
                    command: String::from("printf 'buongiorno'"),
                    description: None,
                    section: Some(String::from("Job section")),
                }),
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
                schedule: String::from("* * * * *"),
                command: String::from("printf 'hello, world'"),
                description: None,
                section: None,
            })]
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
