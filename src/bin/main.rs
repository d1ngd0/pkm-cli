use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use chrono::Local;
use clap::{ArgMatches, Command, arg};
use log::error;
use markdown::{ParseOptions, mdast::Node};
use pkm::{Editor, Result, ZettelBuilder, ZettelIDBuilder, ZettelPathBuf, find_node};
use tera::Context;

fn cli() -> Command {
    Command::new("pkm")
        .about("A PKM management CLI")
        .arg(arg!(REPO: -r --repo <REPO> "The root directory of the pkm").env("PKM_REPO"))
        .subcommand(
            Command::new("zettel")
                .about("Create a new zettel")
                .alias("ztl")
                .arg(arg!(ZETTEL_DIR: --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels"))
                .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl"))
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel").default_value("default"))
                .arg(arg!(MEETING: --meeting "mark the zettel as notes for a meeting"))
                .arg(arg!(FLEETING: --fleeting "mark the zettel as fleeting notes"))
                .arg(arg!(DATE: --date "put the date into the filename"))
                .arg(arg!(NO_EDIT: --"no-edit" "Do not open in an editor once created"))
                .arg(arg!(TITLE: <TITLE> "The title of the zettel"))
                .arg(arg!(VARS: ... "variables for the template (title:\"Hello World\")"))
        )
        .subcommand(
            Command::new("daily")
                .about("open the daily file")
                .alias("day")
                .arg(arg!(DAILY_DIR: --"daily-dir" [DAILY_DIR] "The directory where dailys are stored relative to the repo directory").env("PKM_DAILY_DIR").default_value("daily"))
                .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl"))
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel").default_value("daily"))
                .arg(arg!(VARS: ... "variables for the template (title:\"Hello World\")"))
        )
        .subcommand(
            Command::new("repo")
                .about("run git commands")
                .alias("git")
                .arg(
                    arg!(VARS: [VARS]) // Accept 1 or more args
                    .num_args(1..)
                    .allow_hyphen_values(true)
                    .trailing_var_arg(true)
                )
        )
        .subcommand(
            Command::new("favorites")
                .about("A list of favorites")
                .alias("fvt")
        )
        .subcommand(Command::new("search").about("Finds your relavent data"))
}

fn main() {
    env_logger::init();

    let matches = cli().get_matches();
    let repo = matches.get_one::<String>("REPO").expect("repo required");

    let res = match matches.subcommand() {
        Some(("zettel", sub_matches)) => run_zettel(sub_matches, &repo),
        Some(("daily", sub_matches)) => run_daily(sub_matches, &repo),
        Some(("repo", sub_matches)) => run_repo(sub_matches, &repo),
        Some(("favorites", sub_matches)) => run_favorites(sub_matches, &repo),
        None => run_editor(&matches, &repo),
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    if let Err(err) = res {
        error!("{}", err)
    }
}

// build_zettel_context will build the context to create a new zettel from a template
fn build_context_args(args: &ArgMatches) -> Context {
    let mut context = tera::Context::new();
    // add the title

    if let Some(vars) = args.get_many::<String>("VARS") {
        for value in vars {
            if let Some((key, value)) = value.split_once(":") {
                context.insert(key, value);
            }
        }
    }

    context
}

fn template_dir_path<P>(repo: P, args: &ArgMatches) -> PathBuf
where
    P: AsRef<Path>,
{
    let mut template_dir = PathBuf::new();
    template_dir.push(repo.as_ref());
    template_dir.push(args.get_one::<String>("TEMPLATE_DIR").expect("defaulted"));
    template_dir
}

fn run_editor<P>(_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    Editor::new_from_env("EDITOR", repo.as_ref())
        .file("README.md")
        .exec()?;
    Ok(())
}

fn run_zettel<P>(sub_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let current_date = Local::now();
    let mut context = build_context_args(sub_matches);

    // destination starts with the path to the repo
    let mut destination = PathBuf::new();
    destination.push(repo.as_ref());
    // then add the zettel directory
    destination.push(
        sub_matches
            .get_one::<String>("ZETTEL_DIR")
            .expect("defaulted"),
    );

    // the date directory structure
    destination.push_year_month_day(current_date);
    // the name of the file
    let mut id = ZettelIDBuilder::new(Some(
        sub_matches
            .get_one::<String>("TITLE")
            .expect("title required"),
    ))
    .with_hash();

    if let Some(true) = sub_matches.get_one::<bool>("MEETING") {
        id = id.prefix("meeting");
        id = id.date(current_date)
    }

    if let Some(true) = sub_matches.get_one::<bool>("FLEETING") {
        id = id.prefix("fleeting")
    }

    if let Some(true) = sub_matches.get_one::<bool>("DATE") {
        id = id.date(current_date)
    }

    if let Some(date) = id.get_date() {
        context.insert("daily", date)
    }

    let id = id.to_string()?;

    destination.push_id(&id);

    let template_dir = template_dir_path(repo.as_ref(), sub_matches);

    context.insert(
        "title",
        sub_matches
            .get_one::<String>("TITLE")
            .expect("title required"),
    );

    ZettelBuilder::new(destination.as_path(), template_dir.as_path())
        .template(sub_matches.get_one::<String>("TEMPLATE"))
        .build(&context)?;

    if let Some(true) = sub_matches.get_one::<bool>("NO_EDIT") {
        println!(
            "created zettel: {}",
            destination.as_path().to_string_lossy()
        )
    } else {
        Editor::new_from_env("EDITOR", repo.as_ref())
            .file(destination)
            .exec()?;
    }

    Ok(())
}

fn run_daily<P>(sub_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let current_date = Local::now();

    // destination starts with the path to the repo
    let mut destination = PathBuf::new();
    destination.push(repo.as_ref());
    // then add the zettel directory
    destination.push(
        sub_matches
            .get_one::<String>("DAILY_DIR")
            .expect("defaulted"),
    );

    // the date directory structure
    destination.push_year_month(current_date);
    // the name of the file
    destination.push_id(
        ZettelIDBuilder::new(None)
            .date(current_date)
            .to_string()?
            .as_ref(),
    );

    // if the file already exists just open it and return early
    if destination.as_path().exists() {
        Editor::new_from_env("EDITOR", repo.as_ref())
            .file(destination)
            .exec()?;
        return Ok(());
    }

    let template_dir = template_dir_path(repo.as_ref(), sub_matches);
    let context = build_context_args(sub_matches);

    ZettelBuilder::new(destination.as_path(), template_dir.as_path())
        .template(sub_matches.get_one::<String>("TEMPLATE"))
        .build(&context)?;

    Editor::new_from_env("EDITOR", repo.as_ref())
        .file(destination)
        .exec()?;

    Ok(())
}

fn run_repo<P>(matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    std::process::Command::new("git")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(repo)
        .args(
            matches
                .get_many::<String>("VARS")
                .expect("arguments required"),
        )
        .status()?;

    Ok(())
}

fn run_favorites<P>(_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut favorites = PathBuf::from(repo.as_ref());
    favorites.push("favorites.md");
    let favorites = fs::read_to_string(favorites.as_path())?;

    let opts = ParseOptions::gfm();
    let ast = markdown::to_mdast(&favorites, &opts)?;
    let table = find_node!(&ast, Node::Table);
    dbg!(table);
    Ok(())
}
