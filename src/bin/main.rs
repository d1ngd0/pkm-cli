use std::{
    ffi::OsStr,
    fs::{self, read_to_string},
    io::stdout,
    path::{Path, PathBuf},
    process::Stdio,
};

use chrono::{DateTime, Local, TimeZone};
use clap::{ArgMatches, Command, ValueHint, arg, value_parser};
use clap_complete::aot::{Shell, generate};
use human_date_parser::ParseResult;
use inquire::Text;
use log::error;
use lsp_types::GotoDefinitionResponse::{Array, Link, Scalar};
use markdown::{ParseOptions, mdast::Node};
use pkm::{
    Editor, Error, Finder, FinderItem, PKMBuilder, Result, ZettelIDBuilder, ZettelIndex,
    ZettelReference, first_node, first_within_child,
    lsp::{LSP, StandardRunnerBuilder},
    path_to_id,
};
use regex::Regex;
use tera::Context;
use walkdir::WalkDir;

const DATE_REGEX: &str = "[0-9]{4}-(0[0-9]|1[0-2])-([0-2][0-9]|3[01])";
const ZETTEL_ICON: &str = "󰎚";
const MEETING_ICON: &str = "";
const DATED_ICON: &str = "󰸗";
const FLEETING_ICON: &str = "";

const MEETING_TAG: &str = "meeting";
const FLEETING_TAG: &str = "fleeting";

fn cli() -> Command {
    let default_repo = if cfg!(debug_assertions) {
        "PKM_DEV_REPO"
    } else {
        "PKM_REPO"
    };

    Command::new("pkm")
        .about("A PKM management CLI")
        .arg(arg!(REPO: -r --repo <REPO> "The root directory of the pkm").env(default_repo))
        .subcommand(
            Command::new("zettel")
                .about("Create a new zettel")
                .alias("ztl")
                .arg(arg!(ZETTEL_DIR: --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels").value_hint(ValueHint::DirPath))
                .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl").value_hint(ValueHint::DirPath))
                .arg(arg!(DAILY_DIR: --"daily-dir" [DAILY_DIR] "The directory where dailys are stored relative to the repo directory").env("PKM_DAILY_DIR").default_value("daily").value_hint(ValueHint::DirPath))
                .arg(arg!(IMG_DIR: --"img-dir" [IMG_DIR] "The directory, relative to the root directory, where images are stored").env("PKM_DAILY_DIR").default_value("imgs").value_hint(ValueHint::DirPath))
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
                .arg(arg!(ZETTEL_DIR: --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels").value_hint(ValueHint::DirPath))
                .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl").value_hint(ValueHint::DirPath))
                .arg(arg!(DAILY_DIR: --"daily-dir" [DAILY_DIR] "The directory where dailys are stored relative to the repo directory").env("PKM_DAILY_DIR").default_value("daily").value_hint(ValueHint::DirPath))
                .arg(arg!(IMG_DIR: --"img-dir" <IMG_DIR> "The directory, relative to the root directory, where images are stored").env("PKM_DAILY_DIR").default_value("imgs").value_hint(ValueHint::DirPath))
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel").default_value("daily"))
                .arg(arg!(DATE: [DATE] "Human representation of a date for the dailly").default_value("today"))
                .arg(arg!(NO_EDIT: --"no-edit" "Do not open in an editor once created"))
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
        .subcommand(
            Command::new("index")
                .about("Index the data")
        )
        .subcommand(Command::new("search")
            .about("Finds your relavent data"))

        .subcommand(
            Command::new("script")
                .about("run a helper script in pkm `/scripts` directory")
                .alias("s")
                .arg(
                    arg!(VARS: [VARS]) // Accept 1 or more args
                    .num_args(1..)
                    .allow_hyphen_values(true)
                    .trailing_var_arg(true)
            )
        )
        .subcommand(
            Command::new("completion")
                .arg(
                    arg!(SHELL: --shell <SHELL>)
                    .value_parser(value_parser!(Shell))
                )
                .about("Generate shell completion")
        )
        .subcommand(
            Command::new("image")
            .alias("img")
                .arg(arg!(IMG_DIR: --"img-dir" <IMG_DIR> "The directory, relative to the root directory, where images are stored").env("PKM_DAILY_DIR").default_value("imgs").value_hint(ValueHint::DirPath))
            .arg(arg!(IMG: <IMG>).value_hint(ValueHint::FilePath))
            .arg(arg!(MAX_WIDTH: --"max-width" <WIDTH>).required(false).default_value("1400").value_parser(clap::value_parser!(u32)))
            .arg(arg!(MAX_HEIGHT: --"max-height" <HEIGHT>).required(false).default_value("1000").value_parser(clap::value_parser!(u32)))
            .about("Add an image to the repo and echo the path")
        )
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let matches = cli().get_matches();
    let repo = matches.get_one::<String>("REPO").expect("repo required");

    let res = match matches.subcommand() {
        Some(("zettel", sub_matches)) => run_zettel(sub_matches, &repo),
        Some(("daily", sub_matches)) => run_daily(sub_matches, &repo),
        Some(("repo", sub_matches)) => run_repo(sub_matches, &repo),
        Some(("favorites", sub_matches)) => run_favorites(sub_matches, &repo).await,
        Some(("index", sub_matches)) => run_index(sub_matches, &repo),
        Some(("search", sub_matches)) => run_search(sub_matches, &repo),
        Some(("script", sub_matches)) => run_script(sub_matches, &repo),
        Some(("image", submatches)) => run_image(submatches, &repo),
        Some(("completion", submatches)) => run_completion(submatches),
        None => run_editor(&matches, &repo),
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    if let Err(err) = res {
        error!("{}", err)
    }
}

fn run_image<P: AsRef<Path>>(args: &ArgMatches, repo: P) -> Result<()> {
    let pkm = PKMBuilder::new(&repo).parse_args(args).build()?;
    let current_date = Local::now();

    let img = pkm
        .image()
        .with_date_directory(&current_date)
        .max_width(args.get_one::<u32>("MAX_WIDTH").copied())
        .max_height(args.get_one::<u32>("MAX_HEIGHT").copied())
        .build(args.get_one::<String>("IMG").expect("required"))?;

    println!(
        "{}",
        img.rel_path(&repo)
            .expect("we just put it into that directory")
            .to_string_lossy()
    );
    Ok(())
}

fn run_completion(args: &ArgMatches) -> Result<()> {
    let mut cmd = cli();
    let generator: Shell = args.get_one("SHELL").copied().expect("Required Field");
    let name = cmd.get_name().to_string();
    generate(generator, &mut cmd, &name, &mut stdout());
    Ok(())
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

    if let Some(title) = args.get_one::<String>("TITLE") {
        context.insert("title", title)
    }

    context
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
    let date_reg = Regex::new(DATE_REGEX).expect("must compile");

    let pkm = PKMBuilder::new(&repo).parse_args(sub_matches).build()?;

    let id = ZettelIDBuilder::new()
        .parse_args(sub_matches, &current_date)
        .with_hash()
        .build()?;

    let mut reference_prefix = ZETTEL_ICON;

    if let Some(date) = id.tag_regex(&date_reg) {
        context.insert("daily", date);
        reference_prefix = DATED_ICON;
    }

    if id.has_tag(FLEETING_TAG) {
        reference_prefix = FLEETING_ICON;
    }

    if id.has_tag(MEETING_TAG) {
        reference_prefix = MEETING_ICON;
    }

    let zettel = pkm
        .zettel()
        .with_year_month_day(&current_date)
        .parse_args(sub_matches)
        .id(&id)
        .build(&pkm.tmpl, &context)?;

    // add the reference to the daily
    let reference = ZettelReference::new(&id, reference_prefix);
    let reference: String = reference.into();
    let mut daily = pkm.daily(&current_date)?;
    daily.content()?.append(&reference)?;
    daily.sync()?;

    if let Some(true) = sub_matches.get_one::<bool>("NO_EDIT") {
        println!("{}", zettel.rel_path(repo.as_ref())?.to_string_lossy())
    } else {
        Editor::new_from_env("EDITOR", repo.as_ref())
            .file(zettel.rel_path(repo.as_ref())?)
            .exec()?;
    }

    Ok(())
}

fn parse_human_date(date: &str) -> Result<DateTime<Local>> {
    // this library makes things hard
    let current_date = human_date_parser::from_human_time(date, Local::now().naive_local())?;
    let current_date = match current_date {
        ParseResult::DateTime(datetime) => datetime,
        ParseResult::Date(date) => date.into(),
        ParseResult::Time(_) => Local::now().naive_local(),
    };

    Ok(Local.from_local_datetime(&current_date).unwrap())
}

fn run_daily<P>(sub_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let current_date = parse_human_date(sub_matches.get_one::<String>("DATE").expect("defaulted"))?;
    let pkm = PKMBuilder::new(&repo).parse_args(sub_matches).build()?;
    let daily = pkm.daily(&current_date)?;

    if let Some(true) = sub_matches.get_one::<bool>("NO_EDIT") {
        println!("{}", daily.rel_path(repo.as_ref())?.to_string_lossy())
    } else {
        Editor::new_from_env("EDITOR", repo.as_ref())
            .file(daily.rel_path(repo.as_ref())?)
            .exec()?;
    }

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

fn run_script<P>(matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut arguments = matches
        .get_many::<String>("VARS")
        .expect("arguments required")
        .into_iter();

    let mut command = String::from("./scripts/");
    command.push_str(arguments.next().expect("required"));

    std::process::Command::new(&command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(repo)
        .args(arguments)
        .status()?;

    Ok(())
}

// run_index creates/updates the index
fn run_index<P>(_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let index = ZettelIndex::new(repo.as_ref())?;
    let mut writer = index.doc_indexer()?;

    // TODO: be smarter
    writer.clear()?;

    for doc in WalkDir::new(repo.as_ref()) {
        let doc = match doc {
            Err(err) => {
                error!("issue indexing {}", err);
                continue;
            }
            Ok(v) => v,
        };

        if doc.path().extension() != Some(OsStr::new("md")) {
            continue;
        }

        let id = path_to_id(doc.path());
        writer.process(&id, doc.path()).unwrap_or_else(|err| {
            error!("could not index document {}", err);
            ()
        });
    }

    writer.commit()?;

    Ok(())
}

fn run_search<P>(_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let index = ZettelIndex::new(repo.as_ref())?;
    loop {
        let query = Text::new(" >").with_placeholder("Query").prompt()?;
        let docs = match index.doc_searcher()?.find(&query) {
            Ok(v) => v,
            Err(err) => {
                error!("oops: {}", err);
                continue;
            }
        };

        let mut finder = Finder::new(repo.as_ref());
        for doc in docs {
            let mut full_path = PathBuf::from(repo.as_ref());
            full_path.push(doc.get("uri").expect("schema should have uri"));

            let content = read_to_string(&full_path)?;

            finder.add(
                FinderItem::new(doc.get("uri").expect("schema should have uri"))
                    .with_display(doc.get("title"))
                    .with_syntax_preview(&content, Some("md"), None)?,
            )?;
        }

        if finder.run()? {
            break;
        }
    }

    Ok(())
}

async fn run_favorites<P>(_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let runner = StandardRunnerBuilder::new("markdown-oxide")
        .working_dir(repo.as_ref())
        .spawn()?;
    let mut lsp = LSP::new(runner, repo.as_ref()).await?;

    let mut favorites = PathBuf::from(repo.as_ref());
    favorites.push("favorites.md");
    let fcontent = fs::read_to_string(favorites.as_path())?;

    let opts = ParseOptions::gfm();
    let ast = markdown::to_mdast(&fcontent, &opts)?;
    let table = first_node!(&ast, Node::Table).ok_or(Error::NotFound(String::from(
        "could not find table in favorites",
    )))?;

    let mut iter = table.children.iter();
    iter.next()
        .ok_or(Error::NotFound(String::from("favorite expected a header")))?; // drop the header

    let mut finder = Finder::new(repo.as_ref());
    for row in iter {
        if let Node::TableRow(row) = row {
            let zettel = first_within_child!(0, row, Node::Text).ok_or(Error::NotFound(
                String::from("could not get zettel from favorites"),
            ))?;

            if let Ok(resp) = lsp
                .goto_defintion(
                    favorites.as_path(),
                    zettel.position.as_ref().unwrap().start.line as u32 - 1,
                    zettel.position.as_ref().unwrap().start.column as u32 - 1,
                )
                .await
            {
                match resp {
                    Scalar(location) => finder.add_fq_doc(location.uri)?,
                    Array(locations) => {
                        for location in locations {
                            finder.add_fq_doc(location.uri)?;
                        }
                    }
                    Link(_) => (),
                }
            }
        }
    }

    finder.run()?;

    Ok(())
}
