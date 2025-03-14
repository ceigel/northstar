use itertools::Itertools;
use northstar_client::model::{
    self, Container, ContainerData, ExitStatus, MountResult, Notification, RepositoryId,
    UmountResult,
};
use prettytable::{format, Attr, Cell, Row, Table};
use std::collections::{HashMap, HashSet};
use tokio::time;

pub fn notification(notification: &Notification) {
    match notification {
        Notification::CGroup(container, notification) => {
            println!("container {} memory event {:?}", container, notification)
        }
        Notification::Exit(container, status) => println!(
            "container {} exited with status {}",
            container,
            match status {
                ExitStatus::Exit { code } => format!("exit code {}", code),
                ExitStatus::Signalled { signal } => format!("signalled {}", signal),
            }
        ),
        Notification::Install(container) => println!("installed {}", container),
        Notification::Uninstall(container) => println!("uninstalled {}", container),
        Notification::Started(container) => println!("started {}", container),
        Notification::Shutdown => println!("shutting down"),
    }
}

pub fn list(containers: &HashMap<Container, ContainerData>) {
    let titles = [
        "Name",
        "Version",
        "Repository",
        "Type",
        "Mounted",
        "PID",
        "Uptime",
    ];

    let rows = containers
        .iter()
        .sorted_by_key(|(c, _)| c.name().to_string())
        .sorted_by_key(|(_, d)| d.manifest.init.is_none())
        .map(|(container, data)| {
            [
                Cell::new(container.name().as_ref()).with_style(Attr::Bold),
                Cell::new(&container.version().to_string()),
                Cell::new(&data.repository),
                if data.manifest.init.is_some() {
                    Cell::new("app").with_style(Attr::ForegroundColor(prettytable::color::BLUE))
                } else {
                    Cell::new("resource")
                        .with_style(Attr::ForegroundColor(prettytable::color::GREEN))
                },
                if data.mounted {
                    Cell::new("yes").with_style(Attr::ForegroundColor(prettytable::color::YELLOW))
                } else {
                    Cell::new("no").with_style(Attr::ForegroundColor(prettytable::color::CYAN))
                },
                Cell::new(
                    &data
                        .process
                        .as_ref()
                        .map(|p| p.pid.to_string())
                        .unwrap_or_default(),
                )
                .with_style(Attr::ForegroundColor(prettytable::color::GREEN)),
                Cell::new(
                    &data
                        .process
                        .as_ref()
                        .map(|p| {
                            humantime::format_duration(time::Duration::from_nanos(p.uptime))
                                .to_string()
                        })
                        .unwrap_or_default(),
                ),
            ]
        });

    print_table(titles, rows);
}

pub fn repositories(repositories: &HashSet<RepositoryId>) {
    let iter = repositories
        .iter()
        .sorted_by_key(|i| (*i).clone())
        .map(|i| [Cell::new(i).with_style(Attr::Bold)]);
    print_table(["Name"], iter);
}

pub fn mounts(mounts: &[MountResult]) {
    let iter = mounts.iter().map(|r| match r {
        MountResult::Ok { container } => [
            Cell::new(&container.to_string()).with_style(Attr::Bold),
            Cell::new("ok"),
        ],
        MountResult::Error { container, error } => [
            Cell::new(&container.to_string()).with_style(Attr::Bold),
            Cell::new(&format_err(error)),
        ],
    });
    print_table(["Name", "Result"].iter(), iter);
}

pub fn umounts(mounts: &[UmountResult]) {
    let iter = mounts.iter().map(|r| match r {
        UmountResult::Ok { container } => [
            Cell::new(&container.to_string()).with_style(Attr::Bold),
            Cell::new("ok"),
        ],
        UmountResult::Error { container, error } => [
            Cell::new(&container.to_string()).with_style(Attr::Bold),
            Cell::new(&format_err(error)),
        ],
    });
    print_table(["Name", "Result"], iter);
}

fn format_err(err: &model::Error) -> String {
    match err {
        model::Error::Configuration { context } => format!("invalid configuration: {}", context),
        model::Error::DuplicateContainer { container } => {
            format!("duplicate container name and version {}", container)
        }
        model::Error::InvalidContainer { container } => format!("invalid container {}", container),
        model::Error::InvalidArguments { cause } => format!("invalid arguments {}", cause),
        model::Error::MountBusy { container } => format!("container busy: {}", container),
        model::Error::UmountBusy { container } => format!("container busy: {}", container),
        model::Error::StartContainerStarted { container } => {
            format!("failed to start container {}: already started", container)
        }
        model::Error::StartContainerResource { container } => {
            format!("failed to start container {}: resource", container)
        }
        model::Error::StartContainerMissingResource {
            container,
            resource,
            version,
        } => {
            format!(
                "failed to start container {}: missing resource {} version {}",
                container, resource, version
            )
        }
        model::Error::StartContainerFailed { container, error } => {
            format!("failed to start container {}: {}", container, error)
        }
        model::Error::StopContainerNotStarted { container } => {
            format!("failed to stop container {}: not started", container)
        }
        model::Error::InvalidRepository { repository } => {
            format!("invalid repository {}", repository)
        }
        model::Error::InstallDuplicate { container } => {
            format!("failed to install {}: installed", container)
        }
        model::Error::CriticalContainer { container, status } => {
            format!(
                "critical container {} exited with: {}",
                container,
                match status {
                    ExitStatus::Exit { code } => format!("exit code {}", code),
                    ExitStatus::Signalled { signal } => format!("signaled {}", signal),
                }
            )
        }
        model::Error::Unexpected { error } => error.to_string(),
    }
}

fn print_table<S, T, C, R>(titles: T, rows: R)
where
    S: ToString,
    T: IntoIterator<Item = S>,
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = Cell>,
{
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    let titles = titles
        .into_iter()
        .map(|t| Cell::new(&t.to_string()).with_style(Attr::Bold));
    table.set_titles(Row::new(titles.collect()));

    rows.into_iter()
        .map(|row| Row::new(row.into_iter().collect::<Vec<_>>()))
        .for_each(|r| {
            table.add_row(r);
        });
    table.printstd();
}
