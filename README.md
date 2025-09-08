# Scuttle
A multi-service command-line tool built with Rust for scuttling (synchronizing) your files between your local filesystem and the cloud.

## Features
* Blazing Fast: Written in Rust, Scuttle is designed for speed and safety.

* Multi-Service Support: Connect to popular cloud storage providers. (Currently supporting Google Drive, with more services like OneDrive planned.)

* Multiple Account Support: Manage multiple cloud accounts and configurations seamlessly.

* Simple CLI: A straightforward and intuitive command-line interface makes file syncing a breeze.

## Why Scuttle?
Ever needed a single, unified tool to manage your files across different cloud platforms? Scuttle provides a simple and efficient way to handle your data without juggling multiple applications. It's built for developers and power users who value control, performance, and flexibility.

## Usage
### Upload a file to your configured cloud service
scuttle upload <file_path>

### Download a file from the cloud
scuttle download <file_name>

### List files in a cloud directory
scuttle list

### Add files to staging area
scuttle add <file1> <file2> ...

You can also add all files recursively in a directory:

```bash
scuttle add .
```

### Show status of local files compared to tracked files

```bash
scuttle status
```

### Setup and Multiple Account Support
Run `scuttle setup` to configure your cloud accounts. You can add multiple accounts by running the setup multiple times with different remote names. You can also choose which account to use as the default for operations.

### Example Commands
Run the following commands to get started:

```bash
cargo run -- setup
cargo run -- upload test_folder/test.txt
cargo run -- download test.txt
cargo run -- add README.md
cargo run -- status
```

## Debugging and Credentials Setup
To debug and properly configure Google Drive access, you need to obtain the `credentials.json` file from the Google Cloud Console. [Google Cloud Console OAuth Tutorial](https://developers.google.com/workspace/drive/api/quickstart/python#configure_the_oauth_consent_screen).

Follow these steps to configure the OAuth consent screen and get your credentials:

1. Go to the [GCP](https://console.cloud.google.com/auth/branding)

2. In the Google Cloud console, navigate to Menu > Google Auth platform > Branding.

3. If you haven't configured the Google Auth platform yet, click "Get Started".

4. Under App Information:
   - Enter an App name.
   - Choose a User support email.

5. Click Next.

6. Under Audience, select "Internal".

7. Click Next.

8. Under Contact Information, enter an email address for notifications.

9. Click Next.

10. Under Finish, review the Google API Services User Data Policy, agree to it, and click Continue.

11. Click Create.

12. For now, you can skip adding scopes. When creating an app for use outside your Google Workspace organization, change the User type to "External" and add required authorization scopes.

13. Download the `credentials.json` file and place it in your project directory or the appropriate config directory.

This setup is essential for Scuttle to authenticate and interact with Google Drive.

## Future Plans
Scuttle aims to evolve into a powerful cloud storage version control system, similar to git but for your cloud files and folders.

Upcoming features include:

* **File Sync System:** Manage your folders with commands like `scuttle add`, `scuttle commit`, `scuttle push`, and `scuttle pull` to synchronize changes efficiently.

* **Single File Operations:** Upload, download, and update individual files seamlessly.

* **Multi-Service Support:** After completing the initial core features, support for other cloud services like OneDrive, Dropbox, and SMB will be added.

* **Enhanced Folder Management:** Track changes, manage versions, and collaborate across multiple cloud platforms with ease.

Stay tuned for updates as we build out these exciting features!

## Contributing
We welcome contributions! If you have ideas for new features, bug reports, or want to help with development, please check out our Contributing Guide.

## License
This project is licensed under the AGPL v3 License.
