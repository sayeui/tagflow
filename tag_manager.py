import os
import re
import pandas as pd

class TagManager:
    def __init__(self):
        self.tags_df = pd.DataFrame(columns=['file_path', 'tags'])

    def auto_tag(self, file_path):
        """Automatically tag a file based on its path and name."""
        file_name = os.path.basename(file_path)
        path_parts = file_path.split(os.sep)
        tags = set(path_parts) | set(re.findall(r'\b\w+\b', file_name))
        self.add_tags(file_path, tags)

    def add_tags(self, file_path, tags):
        """Add tags to a file."""
        if file_path in self.tags_df['file_path'].values:
            self.tags_df.loc[self.tags_df['file_path'] == file_path, 'tags'] += list(tags)
        else:
            self.tags_df = self.tags_df.append({'file_path': file_path, 'tags': list(tags)}, ignore_index=True)

    def remove_tags(self, file_path, tags):
        """Remove tags from a file."""
        if file_path in self.tags_df['file_path'].values:
            current_tags = set(self.tags_df.loc[self.tags_df['file_path'] == file_path, 'tags'].values[0])
            updated_tags = current_tags - set(tags)
            self.tags_df.loc[self.tags_df['file_path'] == file_path, 'tags'] = list(updated_tags)

    def get_tags(self, file_path):
        """Get tags for a file."""
        if file_path in self.tags_df['file_path'].values:
            return self.tags_df.loc[self.tags_df['file_path'] == file_path, 'tags'].values[0]
        return []