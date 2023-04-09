use std::{
    collections::HashMap,
    error::Error,
    ffi::CString,
    fs,
    fs::OpenOptions,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use futures::executor::block_on;

use libc::{open, O_RDWR};
use zbus::{
    export::ordered_stream::OrderedStreamExt,
    zvariant::{Array, Fd, Value},
    Connection,
};

use crate::{
    common::shell::FileFilter,
    linux::{
        dbus::{file_chooser::FileChooserProxy, open_uri::OpenURIProxy, request::RequestProxy},
        errno::syscall,
    },
    utils::scoped::Scoped,
};

pub fn is_supported() -> bool {
    let get_version = || -> Result<(u32, u32), Box<dyn Error>> {
        block_on(async {
            let conn = Connection::session().await?;

            let open_uri = OpenURIProxy::builder(&conn)
                .destination("org.freedesktop.portal.Desktop")?
                .path("/org/freedesktop/portal/desktop")?
                .build()
                .await?;
            let open_uri_ver = open_uri.version().await?;

            let file_chooser = FileChooserProxy::builder(&conn)
                .destination("org.freedesktop.portal.Desktop")?
                .path("/org/freedesktop/portal/desktop")?
                .build()
                .await?;
            let file_chooser_ver = file_chooser.version().await?;

            Ok((open_uri_ver, file_chooser_ver)) as Result<(u32, u32), Box<dyn Error>>
        })
    };

    match get_version() {
        Ok((open_uri_ver, file_chooser_ver)) => open_uri_ver >= 1 && file_chooser_ver >= 1,
        Err(_) => false,
    }
}

pub fn run_pick_file(window: i64, title: &str, filters: &[FileFilter]) -> Result<Option<PathBuf>, Box<dyn Error>> {
    block_on(async {
        let conn = Connection::session().await?;

        let filters = filters
            .iter()
            .map(|f| (&f.label, f.extensions.iter().map(|ext| (0u32, ext)).collect::<Vec<_>>()))
            .collect::<Vec<_>>();

        let filters = Value::Array(Array::from(filters));

        let mut options: HashMap<&str, Value> = HashMap::new();
        options.insert("filters", filters);

        let ret = FileChooserProxy::builder(&conn)
            .destination("org.freedesktop.portal.Desktop")?
            .path("/org/freedesktop/portal/desktop")?
            .build()
            .await?
            .open_file(&format!("x11:{:x}", window), title, options)
            .await?;

        let request_proxy = RequestProxy::builder(&conn)
            .destination("org.freedesktop.portal.Desktop")?
            .path(ret)?
            .build()
            .await?;

        loop {
            if let Some(response) = request_proxy.receive_response().await?.next().await {
                let args = response.args()?;

                if args.response == 0 {
                    if let Value::Array(uris) = &args.results["uris"] {
                        if let Some(Value::Str(uri)) = uris.first() {
                            if let Some(path) = uri.to_string().strip_prefix("file://") {
                                return Ok(Some(PathBuf::from(path)));
                            }
                        }
                    }
                }

                return Ok(None);
            }
        }
    })
}

pub fn run_launch_file(window: i64, file: &str) -> Result<(), Box<dyn Error>> {
    block_on(async {
        let conn = Connection::session().await?;

        let file = CString::new(file)?;
        let fd = unsafe { Scoped::new_fd(syscall(|| open(file.as_ptr(), O_RDWR))?) };

        OpenURIProxy::builder(&conn)
            .destination("org.freedesktop.portal.Desktop")?
            .path("/org/freedesktop/portal/desktop")?
            .build()
            .await?
            .open_file(&format!("x11:{:x}", window), Fd::from(*fd), HashMap::new())
            .await?;

        Ok(())
    })
}

fn refresh_desktop_database(home_dir: &Path) {
    Command::new("xdg-desktop-menu")
        .args(["forceupdate"])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .ok();

    Command::new("gtk-update-icon-cache")
        .args(&["-t", home_dir.join(".local/share/icons/hicolor").to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok();
}

pub fn install_icon(name: &str, data: &[u8]) -> Result<(), Box<dyn Error>> {
    let file = ico::IconDir::read(Cursor::new(data))?;

    let home_dir = if let Some(home_dir) = home::home_dir() {
        home_dir
    } else {
        return Err("unable to find home directory".into());
    };

    for entry in file.entries() {
        let image = entry.decode()?;

        let png_path = home_dir
            .join(".local/share/icons/hicolor")
            .join(format!("{}x{}", image.width(), image.height()))
            .join("apps")
            .join(name)
            .with_extension("png");

        fs::create_dir_all(png_path.parent().unwrap())?;

        let png_file = OpenOptions::new().write(true).create(true).truncate(true).open(png_path)?;

        image.write_png(png_file)?;
    }

    refresh_desktop_database(&home_dir);

    Ok(())
}

pub fn install_shortcut(
    app_id: &str,
    name: &str,
    icon: &str,
    executable: &str,
    arguments: &[String],
) -> Result<(), Box<dyn Error>> {
    let content = [
        "[Desktop Entry]".to_owned(),
        "Name=".to_owned() + name,
        "Comment=".to_owned() + name,
        "Exec=".to_owned() + executable + " " + &arguments.join(" "),
        "Terminal=false".to_owned(),
        "Type=Application".to_owned(),
        "Icon=".to_owned() + icon,
    ]
    .join("\n");

    let home_dir = if let Some(home_dir) = home::home_dir() {
        home_dir
    } else {
        return Err("unable to find home directory".into());
    };

    let desktop_path = home_dir
        .join(".local/share/applications")
        .join(app_id)
        .with_extension("desktop");

    fs::create_dir_all(desktop_path.parent().unwrap())?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(desktop_path)?;

    file.write_all(content.as_bytes())?;

    refresh_desktop_database(&home_dir);

    Ok(())
}

pub fn uninstall_shortcut(app_id: &str, _: &str) -> Result<(), Box<dyn Error>> {
    let home_dir = if let Some(home_dir) = home::home_dir() {
        home_dir
    } else {
        return Err("unable to find home directory".into());
    };

    let desktop_path = home_dir
        .join(".local/share/applications")
        .join(app_id)
        .with_extension("desktop");

    fs::remove_file(desktop_path)?;

    refresh_desktop_database(&home_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        common::shell::FileFilter,
        linux::{
            shell::{install_icon, install_shortcut, run_pick_file, uninstall_shortcut},
            testdata,
        },
    };

    #[test]
    pub fn test_pick_file() -> Result<(), Box<dyn Error>> {
        let ret = run_pick_file(
            0,
            "Open File...",
            &[FileFilter {
                label: "All Files".to_string(),
                extensions: vec!["*".to_string()],
            }],
        )?;

        println!("{:?}", ret);

        Ok(())
    }

    const TEST_ICON_NAME: &str = "clash-multiplatform-compat-library";
    const TEST_ICON_PATH: &str = "clash-multiplatform.ico";

    #[test]
    pub fn test_install_icon() -> Result<(), Box<dyn Error>> {
        let data = testdata::TestData::get(TEST_ICON_PATH).unwrap().data;

        install_icon(TEST_ICON_NAME, &data)
    }

    const TEST_APP_ID: &str = "clash-compat-library";
    const TEST_APP_NAME: &str = "Clash Compat Library (Test)";

    #[test]
    pub fn test_install_shortcut() -> Result<(), Box<dyn Error>> {
        let self_exe = std::env::current_exe()?;

        install_shortcut(TEST_APP_ID, TEST_APP_NAME, TEST_ICON_NAME, self_exe.to_str().unwrap(), &[])
    }

    #[test]
    pub fn test_uninstall_shortcut() -> Result<(), Box<dyn Error>> {
        uninstall_shortcut(TEST_APP_ID, TEST_APP_NAME)
    }
}