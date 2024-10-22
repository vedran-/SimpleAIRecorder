use x_win::{get_active_window, XWinError};

pub fn get_active_window_info() -> String {
    match get_active_window() {
        Ok(active_window) => format!("Active app: {}\nTitle: {}\nExec: {}\nPath: {}", 
            active_window.info.name, active_window.title, active_window.info.exec_name, active_window.info.path),
        Err(XWinError) => {
            eprintln!("Error occurred while getting the active window title");
            "Unknown".to_string()
        }
    }
}
