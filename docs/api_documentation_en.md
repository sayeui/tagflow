# File Manager API Documentation

## Overview
This document provides a detailed description of the File Manager API, including endpoints for uploading files, managing tags, and importing folders.

## Endpoints

### 1. Upload a File
- **Endpoint:** `/upload`
- **Method:** `POST`
- **Description:** Upload a file to the object storage.
- **Request:**
  - **Form Data:**
    - `file` (required): The file to be uploaded.
- **Response:**
  - **Success (200):**
  
### 2. Get File Tags
- **Endpoint:** `/tags/<path:file_path>`
- **Method:** `GET`
- **Description:** Get tags for a file.
- **Response:**
  - **Success (200):**
    ```json
    {
      "tags": ["tag1", "tag2"]
    }
    ```

### 3. Get File Content
- **Endpoint:** `/files/<path:object_name>`
- **Method:** `GET`
- **Description:** Get a file from the object storage.
- **Response:**
  - **Success (200):**
    ```json
    {
      "content": "file content"
    }
    ```
  - **Error (500):**
    ```json
    {
      "error": "error message"
    }
    ```

### 4. Import a Folder
- **Endpoint:** `/folders/import`
- **Method:** `POST`
- **Description:** Import all files from a folder into the object storage.
- **Request:**
  - **Form Data:**
    - `folder_path` (required): The folder path to be imported.
- **Response:**
  - **Success (200):**
    ```json
    {
      "message": "Folder imported successfully"
    }
    ```

### 5. Add File Tags
- **Endpoint:** `/tags`
- **Method:** `POST`
- **Description:** Add tags to a file.
- **Request:**
  - **Form Data:**
    - `file_path` (required): The file path.
    - `tags` (required): A list of tags.
- **Response:**
  - **Success (200):**
    ```json
    {
      "message": "Tags added successfully"
    }
    ```

### 6. Remove File Tags
- **Endpoint:** `/tags`
- **Method:** `DELETE`
- **Description:** Remove tags from a file.
- **Request:**
  - **Form Data:**
    - `file_path` (required): The file path.
    - `tags` (required): A list of tags.
- **Response:**
  - **Success (200):**
    ```json
    {
      "message": "Tags removed successfully"
    }
    ```