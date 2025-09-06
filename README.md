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

### Setup and Multiple Account Support
Run `scuttle setup` to configure your cloud accounts. You can add multiple accounts by running the setup multiple times with different remote names. You can also choose which account to use as the default for operations.

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
