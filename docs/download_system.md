# Download System

## Overview

The download subsystem provides robust and reliable artifact retrieval with integrity verification.

## Features

- **Resumable downloads** - Resumes partial downloads using HTTP Range headers
- **Retry logic** - Configurable retry attempts (default: 3) for network failures
- **Checksum verification** - SHA-256 verification of all downloaded artifacts
- **Streaming** - Downloads are streamed to disk to minimize memory usage

## Download Workflow

1. Discover artifact version and download URL
2. Initiate download (resume from partial file if exists)
3. Retry on network failure up to configured limit
4. Verify SHA-256 checksum against expected value
5. Store verified artifact in cache
6. Update metadata records

## Security

Artifacts are **never** executed unless integrity verification succeeds. Failed checksum verification results in deletion of the downloaded file and an error.

## API

### DownloadManager
- `new(downloads_dir)` - Create manager with download directory
- `with_retries(n)` - Configure retry count
- `download(url, expected_sha256)` - Download and verify artifact

### Utility Functions
- `compute_sha256(data)` - Compute SHA-256 hash of byte data
- `verify_checksum(data, expected)` - Verify data against expected hash
