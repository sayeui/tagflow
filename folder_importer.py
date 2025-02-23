import os
import shutil

class FolderImporter:
    def __init__(self, file_manager, tag_manager):
        self.file_manager = file_manager
        self.tag_manager = tag_manager

    def import_folder(self, folder_path, base_path=''):
        """Import all files from a folder into the object storage."""
        for root, dirs, files in os.walk(folder_path):
            for file in files:
                file_path = os.path.join(root, file)
                relative_path = os.path.relpath(file_path, folder_path)
                object_name = os.path.join(base_path, relative_path)
                self.file_manager.upload_file(file_path, object_name)
                self.tag_manager.auto_tag(file_path)

    def delete_folder(self, folder_path):
        """Delete all files from a folder in the object storage."""
        for root, dirs, files in os.walk(folder_path, topdown=False):
            for file in files:
                file_path = os.path.join(root, file)
                object_name = os.path.relpath(file_path, folder_path)
                self.file_manager.delete_file(object_name)
            for dir in dirs:
                dir_path = os.path.join(root, dir)
                shutil.rmtree(dir_path)