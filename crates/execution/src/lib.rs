use artifact::LaunchConfig;
use domain::LauncherError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tracing::info;

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionFormat {
    Exe,
    PowerShell,
    Batch,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub exit_code: Option<i32>,
    pub pid: u32,
}

pub struct ExecutionManager {
    working_dir: Option<PathBuf>,
}

impl ExecutionManager {
    pub fn new() -> Self {
        Self { working_dir: None }
    }

    #[allow(dead_code)]
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    pub fn detect_format(path: &Path) -> Result<ExecutionFormat, LauncherError> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match extension.as_str() {
            "exe" => Ok(ExecutionFormat::Exe),
            "ps1" => Ok(ExecutionFormat::PowerShell),
            "bat" | "cmd" => Ok(ExecutionFormat::Batch),
            _ => Err(LauncherError::Execution(format!(
                "Unsupported execution format: .{}",
                extension
            ))),
        }
    }

    pub fn execute(
        &self,
        executable_path: &Path,
        launch_config: &LaunchConfig,
    ) -> Result<ExecutionResult, LauncherError> {
        if !executable_path.exists() {
            return Err(LauncherError::Execution(format!(
                "Executable not found: {}",
                executable_path.display()
            )));
        }

        let format = Self::detect_format(executable_path)?;
        info!(
            "Executing {:?} format: {:?}",
            executable_path.display(),
            format
        );

        let mut command = self.build_command(executable_path, &format);

        if let Some(args) = &launch_config.args {
            command.args(args);
        }

        if let Some(env) = &launch_config.env {
            for (key, value) in env {
                command.env(key, value);
            }
        }

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }

        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        let child = command
            .spawn()
            .map_err(|e| LauncherError::Execution(format!("Failed to start process: {}", e)))?;

        let pid = child.id();
        info!("Process started with PID {}", pid);

        Ok(ExecutionResult {
            exit_code: None,
            pid,
        })
    }

    pub fn execute_and_wait(
        &self,
        executable_path: &Path,
        launch_config: &LaunchConfig,
    ) -> Result<ExecutionResult, LauncherError> {
        if !executable_path.exists() {
            return Err(LauncherError::Execution(format!(
                "Executable not found: {}",
                executable_path.display()
            )));
        }

        let format = Self::detect_format(executable_path)?;

        let mut command = self.build_command(executable_path, &format);

        if let Some(args) = &launch_config.args {
            command.args(args);
        }

        if let Some(env) = &launch_config.env {
            for (key, value) in env {
                command.env(key, value);
            }
        }

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }

        let output = command
            .output()
            .map_err(|e| LauncherError::Execution(format!("Failed to run process: {}", e)))?;

        let exit_code = output.status.code();
        info!("Process completed with exit code {:?}", exit_code);

        Ok(ExecutionResult {
            exit_code,
            pid: 0,
        })
    }

    fn build_command(&self, executable_path: &Path, format: &ExecutionFormat) -> std::process::Command {
        match format {
            ExecutionFormat::Exe => std::process::Command::new(executable_path),
            ExecutionFormat::PowerShell => {
                let mut cmd = std::process::Command::new("powershell.exe");
                cmd.args([
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &executable_path.display().to_string(),
                ]);
                cmd
            }
            ExecutionFormat::Batch => {
                let mut cmd = std::process::Command::new("cmd.exe");
                cmd.args(["/C", &executable_path.display().to_string()]);
                cmd
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_exe() {
        let format = ExecutionManager::detect_format(Path::new("tool.exe")).unwrap();
        assert_eq!(format, ExecutionFormat::Exe);
    }

    #[test]
    fn test_detect_format_ps1() {
        let format = ExecutionManager::detect_format(Path::new("script.ps1")).unwrap();
        assert_eq!(format, ExecutionFormat::PowerShell);
    }

    #[test]
    fn test_detect_format_bat() {
        let format = ExecutionManager::detect_format(Path::new("run.bat")).unwrap();
        assert_eq!(format, ExecutionFormat::Batch);
    }

    #[test]
    fn test_detect_format_unsupported() {
        let result = ExecutionManager::detect_format(Path::new("file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_missing_file() {
        let manager = ExecutionManager::new();
        let config = LaunchConfig {
            executable: "nonexistent.exe".into(),
            args: None,
            env: None,
        };
        let result = manager.execute(Path::new("nonexistent.exe"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_execution_manager_with_working_dir() {
        let manager = ExecutionManager::new().with_working_dir(PathBuf::from("/tmp"));
        assert_eq!(manager.working_dir, Some(PathBuf::from("/tmp")));
    }
}
