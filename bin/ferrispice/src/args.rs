use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    about = "parse, validate, and assemble a circuit netlist",
    long_about = "parse, validate, and assemble a circuit netlist.\n\n\
        A .dc record in the netlist requests linear DC analysis and prints the MNA \
        unknown vector: non-ground node voltages and explicit branch currents.\n\n\
        Positive current flows from an element's first terminal to its second."
)]
pub(crate) struct Args {
    /// path to the circuit netlist
    #[arg(value_name = "netlist")]
    pub path: PathBuf,

    /// print the MNA matrices G, C, b and unknown ordering
    #[arg(long)]
    pub dump_mna: bool,

    /// print the LU factors L, U and row-permutation matrix P for G
    #[arg(long)]
    pub dump_lu: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(arguments: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(std::iter::once("ferrispice").chain(arguments.iter().copied()))
    }

    #[test]
    fn flag_may_precede_or_follow_the_path() {
        for arguments in [["a.cir", "--dump-mna"], ["--dump-mna", "a.cir"]] {
            let args = parse_strs(&arguments).unwrap();
            assert_eq!(args.path, PathBuf::from("a.cir"));
            assert!(args.dump_mna);
        }
        assert!(!parse_strs(&["a.cir"]).unwrap().dump_mna);
    }

    #[test]
    fn lu_flag_is_independent_and_can_be_combined_with_mna() {
        for arguments in [["a.cir", "--dump-lu"], ["--dump-lu", "a.cir"]] {
            let args = parse_strs(&arguments).unwrap();
            assert!(args.dump_lu);
            assert!(!args.dump_mna);
        }
        let args = parse_strs(&["--dump-mna", "a.cir", "--dump-lu"]).unwrap();
        assert!(args.dump_mna && args.dump_lu);
        assert!(!parse_strs(&["a.cir"]).unwrap().dump_lu);
    }

    #[test]
    fn rejects_unknown_options_and_extra_paths() {
        assert!(parse_strs(&["a.cir", "--verbose"]).is_err());
        assert!(parse_strs(&["a.cir", "b.cir"]).is_err());
        assert!(parse_strs(&["--dump-mna"]).is_err());
        assert!(parse_strs(&["--dump-lu"]).is_err());
        assert_eq!(
            parse_strs(&["a.cir", "--help"]).unwrap_err().kind(),
            clap::error::ErrorKind::DisplayHelp
        );
    }
}
