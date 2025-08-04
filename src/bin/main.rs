use std::path::{Path, PathBuf};

use chrono::Datelike;
use clap::{ArgMatches, Command, arg};
use convert_case::{Case, Casing};
use log::error;
use pkm::{Result, ZettelBuilder};
use sha1::{Digest, Sha1};

fn cli() -> Command {
    Command::new("pkm")
        .about("A PKM management CLI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(arg!(REPO: <REPO> "The root directory of the pkm").env("PKM_REPO"))
        .subcommand(
            Command::new("zettel")
                .about("Create a new zettel")
                .arg(arg!(ZETTEL_DIR: -z --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels"))
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel"))
                .arg(arg!(-e --edit [EDIT] "Open the zettel in your $EDITOR after creation"))
                .arg(arg!(TITLE: <TITLE> "The title of the zettel"))
                .arg(arg!(VARS: ... "variables for the template (title:\"Hello World\")"))
                .arg_required_else_help(true),
        )
        .subcommand(Command::new("search").about("Finds your relavent data"))
}

fn main() {
    let matches = cli().get_matches();

    let res = match matches.subcommand() {
        Some(("zettel", sub_matches)) => run_zettel(
            sub_matches,
            matches.get_one::<String>("REPO").expect("required"),
        ),

        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    if let Err(err) = res {
        error!("{}", err)
    }

    // Continued program logic goes here...
}

fn run_zettel<P>(sub_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut context = tera::Context::new();
    let title = sub_matches.get_one::<String>("TITLE").expect("required");
    // add the title
    context.insert("title", title);

    if let Some(vars) = sub_matches.get_many::<String>("VARS") {
        for value in vars {
            if let Some((key, value)) = value.split_once(":") {
                context.insert(key, value)
            }
        }
    }

    // destination starts with the path to the repo
    let mut destination = PathBuf::new();
    destination.push(repo);
    // then add the zettel directory
    destination.push(
        sub_matches
            .get_one::<String>("ZETTEL_DIR")
            .expect("defaulted"),
    );
    // the date directory structure
    let current_date = chrono::Utc::now();
    destination.push(current_date.year().to_string());
    destination.push(current_date.month().to_string());
    destination.push(current_date.day().to_string());
    // the name of the file
    let mut id = title.to_case(Case::Snake);
    id.push_str("-");

    let mut hash = Sha1::new();
    hash.update(current_date.to_rfc3339().as_bytes());
    let hash = hex::encode(hash.finalize()).to_string();
    id.push_str(&hash[0..8]);
    id.push_str(".md");
    destination.push(&id);

    ZettelBuilder::new(destination.as_path())
        .template(sub_matches.get_one::<String>("TEMPLATE"))
        .build(&context)

    // TODO: open the editor if the user wants to
}
