use std::process::Command;
use std::path::Path;
use std::os::windows::process::CommandExt;
use anyhow::{Result, Context};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct Launcher;

impl Launcher {
    pub fn launch(exe_path: &str, login: &str, password: &str, character: &str, seed: &str, use_launcher: bool, server_type: &str) -> Result<u32> {
        let path = Path::new(exe_path);
        
        // Determine parent directory. If empty (just filename), use "."
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        
        // Use just the filename for the start command, since we set current_dir to parent
        // This avoids issues with spaces in full paths if not handled perfectly, 
        // and matches "doing everything next to itself".
        let game_dir = parent; 
        
        let userdata_dir = game_dir.join("userdata");
        let source_ini = format!("server_{}.ini", server_type.to_lowercase());
        let src_path = userdata_dir.join(&source_ini);
        let dst_path = userdata_dir.join("currentserver.ini");

        if src_path.exists() {
            let _ = std::fs::copy(&src_path, &dst_path);
        }

        if use_launcher {
            let launcher_exe = parent.join("launcher.exe");
            let mut args = vec![];
            
            // Re-adding the game exe as first arg often required by patchers
            args.push(path.to_string_lossy().to_string());
            
            if !seed.is_empty() {
                args.push(format!("--seed={}", seed));
            }
            
            args.extend_from_slice(&[
                "startbypatcher".to_string(),
                "nocheck".to_string(),
                "game:cpw".to_string(),
                "console:1".to_string(),
                format!("user:{}", login),
                format!("pwd:{}", password),
                format!("role:{}", character),
            ]);

            let child = Command::new(&launcher_exe)
                .args(&args)
                .current_dir(parent)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .with_context(|| format!("Failed to spawn launcher: {:?}", launcher_exe))?;
            
            Ok(child.id())
        } else {
            // Direct launch
            // User example: start ElementClient.exe startbypatcher game:cpw console:1 user:ryueewuo pwd:Yebani
            
            if !parent.exists() {
                return Err(anyhow::anyhow!("Game directory not found: {:?}", parent));
            }
            
            let mut final_args = vec![
                 "startbypatcher".to_string(),
                 "nocheck".to_string(),
                 "game:cpw".to_string(),
                 "console:1".to_string(),
            ];

            if !seed.is_empty() {
                final_args.push(format!("--seed={}", seed));
            }

            final_args.push(format!("user:{}", login));
            final_args.push(format!("pwd:{}", password));

            if !character.is_empty() {
                final_args.push(format!("role:{}", character));
            }
            
            let child = Command::new(path)
                .args(&final_args)
                .current_dir(parent)
                .creation_flags(CREATE_NO_WINDOW) 
                .spawn()
                .with_context(|| format!("Failed to spawn direct game process: {:?}", path))?;

            Ok(child.id())
        }
    }
}
