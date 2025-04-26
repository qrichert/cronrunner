/// _DJB2_ hash function.
///
/// _DJB2_[^1] is a very lightweight hash function, with rare collisions
/// and good distribution[^2], created by Daniel J. Bernstein.
///
/// [^1]: <http://www.cse.yorku.ca/~oz/hash.html#djb2>
/// [^2]: <https://softwareengineering.stackexchange.com/a/145633>
#[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
pub fn djb2(input: impl AsRef<[u8]>) -> u64 {
    let mut hash: u64 = 5381;
    for byte in input.as_ref() {
        // hash = ((hash << 5) + hash) + byte (= hash * 33 + byte)
        hash = hash
            .wrapping_shl(5)
            .wrapping_add(hash)
            .wrapping_add(u64::from(*byte));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn djb2_regular() {
        assert_eq!(djb2("Hello"), 210_676_686_969);
        assert_eq!(djb2("Hello!"), 6_952_330_670_010);
        assert_eq!(
            djb2("0 0 * * * /path/to/job1.sh"),
            9_456_279_710_372_377_045
        );
    }

    #[test]
    fn djb2_empty_input() {
        assert_eq!(djb2(""), 5381);
    }

    #[test]
    fn djb2_single_characters() {
        assert_eq!(djb2("a"), 177_670);
        assert_eq!(djb2("Z"), 177_663);
        assert_eq!(djb2("0"), 177_621);
        assert_eq!(djb2("1"), 177_622);
    }

    #[test]
    fn djb2_special_characters() {
        assert_eq!(djb2("!@#$%^&*()"), 8_243_049_648_544_081_841);
        assert_eq!(djb2("cron job: $PATH=/usr/bin"), 16_755_231_330_726_877_035);
    }

    #[test]
    fn djb2_long_string() {
        let long_input = "a".repeat(10_000);
        assert_eq!(djb2(&long_input), 8_050_715_442_701_314_837);
    }

    #[test]
    fn djb2_unicode_characters() {
        assert_eq!(djb2("🌑🌒🌓🌔🌕🌖🌗🌘"), 13_910_177_063_547_092_097);
    }

    #[test]
    fn djb2_case_sensitive() {
        assert_ne!(djb2("Hello"), djb2("hello"));
    }
}
