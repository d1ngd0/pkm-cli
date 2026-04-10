use std::{
    ffi::OsStr,
    fs::{self, read_to_string},
    io::stdout,
    path::{PathBuf, absolute},
    process::{ExitCode, Stdio},
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
    Editor, Error, Finder, FinderItem, PKM, PKMBuilder, Result, ZettelIDBuilder, ZettelIndex,
    ZettelReference, first_node, first_within_child, path_to_id,
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
        .arg(arg!(REFERENCE_FILE: --"repo-reference-file" <REFERENCE_FILE> "Find the root git repo from the reference file"))
        .arg(arg!(ZETTEL_DIR: --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels").value_hint(ValueHint::DirPath))
        .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl").value_hint(ValueHint::DirPath))
        .arg(arg!(DAILY_DIR: --"daily-dir" [DAILY_DIR] "The directory where dailys are stored relative to the repo directory").env("PKM_DAILY_DIR").default_value("daily").value_hint(ValueHint::DirPath))
        .arg(arg!(IMG_DIR: --"img-dir" [IMG_DIR] "The directory, relative to the root directory, where images are stored").env("PKM_DAILY_DIR").default_value("imgs").value_hint(ValueHint::DirPath))
        .subcommand(
            Command::new("zettel")
                .about("Create a new zettel")
                .alias("ztl")
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel").default_value("default"))
                .arg(arg!(MEETING: --meeting "mark the zettel as notes for a meeting"))
                .arg(arg!(FLEETING: --fleeting "mark the zettel as fleeting notes"))
                .arg(arg!(DATE: --date "put the date into the filename"))
                .arg(arg!(HASH: --hash "put a hash in the filename"))
                .arg(arg!(NO_EDIT: --"no-edit" "Do not open in an editor once created"))
                .arg(arg!(TITLE: <TITLE> "The title of the zettel"))
                .arg(arg!(VARS: ... "variables for the template (title:\"Hello World\")"))
        )
        .subcommand(
            Command::new("daily")
                .about("open the daily file")
                .alias("day")
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
                .arg(arg!(IMG: <IMG>).value_hint(ValueHint::FilePath))
                .arg(arg!(MAX_WIDTH: --"max-width" <WIDTH>).required(false).default_value("1400").value_parser(clap::value_parser!(u32)))
                .arg(arg!(MAX_HEIGHT: --"max-height" <HEIGHT>).required(false).default_value("1000").value_parser(clap::value_parser!(u32)))
                .about("Add an image to the repo and echo the path")
        )
        .subcommand(
            Command::new("move")
                .arg(arg!(ZTL: <ZTL>).value_hint(ValueHint::FilePath))
                .arg(arg!(REPO: <REPO>).value_hint(ValueHint::DirPath))
                .about("Move a zettel from one repo to a different repo")
        )
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::init();

    let matches = cli().get_matches();

    let repo = repo_from_reference(
        matches
            .get_one::<String>("REFERENCE_FILE")
            .map(|s| s.as_str()),
    )
    .or_else(|| matches.get_one::<String>("REPO").map(PathBuf::from))
    .expect("repo required");

    let pkm = PKMBuilder::new(&repo).parse_args(&matches).build();

    let pkm = match pkm {
        Err(err) => {
            error!("{}", err);
            return ExitCode::FAILURE;
        }
        Ok(val) => val,
    };

    let res = match matches.subcommand() {
        Some(("zettel", sub_matches)) => run_zettel(sub_matches, &pkm),
        Some(("daily", sub_matches)) => run_daily(sub_matches, &pkm),
        Some(("repo", sub_matches)) => run_repo(sub_matches, &pkm),
        Some(("favorites", sub_matches)) => run_favorites(sub_matches, &pkm).await,
        Some(("index", sub_matches)) => run_index(sub_matches, &pkm),
        Some(("search", sub_matches)) => run_search(sub_matches, &pkm),
        Some(("script", sub_matches)) => run_script(sub_matches, &pkm),
        Some(("image", submatches)) => run_image(submatches, &pkm),
        Some(("move", submatches)) => run_move(submatches, &pkm),
        Some(("completion", submatches)) => run_completion(submatches),
        None => run_editor(&matches, &pkm),
        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    if let Err(err) = res {
        error!("{}", err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run_move(args: &ArgMatches, pkm: &PKM) -> Result<()> {}

fn run_image(args: &ArgMatches, pkm: &PKM) -> Result<()> {
    let current_date = Local::now();

    let img = pkm
        .image()
        .with_date_directory(&current_date)
        .max_width(args.get_one::<u32>("MAX_WIDTH").copied())
        .max_height(args.get_one::<u32>("MAX_HEIGHT").copied())
        .build(args.get_one::<String>("IMG").expect("required"))?;

    println!("{}", img.path().to_string_lossy());
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

fn run_editor(_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    Editor::new_from_env("EDITOR", pkm.root.as_path())
        .file("README.md")
        .exec()?;
    Ok(())
}

fn run_zettel(sub_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    let current_date = Local::now();
    let mut context = build_context_args(sub_matches);
    let date_reg = Regex::new(DATE_REGEX).expect("must compile");

    let id = ZettelIDBuilder::new()
        .parse_args(sub_matches, &current_date)
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
        println!("{}", zettel.path().to_string_lossy())
    } else {
        Editor::new_from_env("EDITOR", pkm.root.as_path())
            .file(zettel.rel_path(pkm.root.as_path())?)
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

fn run_daily(sub_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    let current_date = parse_human_date(sub_matches.get_one::<String>("DATE").expect("defaulted"))?;
    let daily = pkm.daily(&current_date)?;

    if let Some(true) = sub_matches.get_one::<bool>("NO_EDIT") {
        println!("{}", daily.path().to_string_lossy())
    } else {
        Editor::new_from_env("EDITOR", pkm.root.as_path())
            .file(daily.rel_path(pkm.root.as_path())?)
            .exec()?;
    }

    Ok(())
}

fn run_repo(matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    std::process::Command::new("git")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(pkm.root.as_path())
        .args(
            matches
                .get_many::<String>("VARS")
                .expect("arguments required"),
        )
        .status()?;

    Ok(())
}

fn run_script(matches: &ArgMatches, pkm: &PKM) -> Result<()> {
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
        .current_dir(pkm.root.as_path())
        .args(arguments)
        .status()?;

    Ok(())
}

// run_index creates/updates the index
fn run_index(_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    let index = ZettelIndex::new(pkm.root.as_path())?;
    let mut writer = index.doc_indexer()?;

    // TODO: be smarter
    writer.clear()?;

    for doc in WalkDir::new(pkm.root.as_path()) {
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

fn run_search(_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    let index = ZettelIndex::new(pkm.root.as_path())?;
    loop {
        let query = Text::new(" >").with_placeholder("Query").prompt()?;
        let docs = match index.doc_searcher()?.find(&query) {
            Ok(v) => v,
            Err(err) => {
                error!("oops: {}", err);
                continue;
            }
        };

        let mut finder = Finder::new(pkm.root.as_path());
        for doc in docs {
            let mut full_path = PathBuf::from(pkm.root.as_path());
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

async fn run_favorites(_matches: &ArgMatches, pkm: &PKM) -> Result<()> {
    let mut favorites = PathBuf::from(pkm.root.as_path());
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

    let mut lsp = pkm.lsp().await?;

    let mut finder = Finder::new(pkm.root.as_path());
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

fn repo_from_reference(refer: Option<&str>) -> Option<PathBuf> {
    let refer = if let Some(refer) = refer {
        refer
    } else {
        return None;
    };

    let mut buf = PathBuf::from(refer);
    while buf.pop() {
        let mut git_path = buf.clone();
        git_path.push(".git");
        if git_path.exists() {
            return Some(buf);
        }
    }

    return None;
}
