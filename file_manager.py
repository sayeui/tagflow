import os
import hashlib
import boto3
from botocore.exceptions import NoCredentialsError
import yaml

class FileManager:
    def __init__(self, bucket_name, region_name, endpoint_url=None):
        if endpoint_url:
            self.s3_client = boto3.client('s3', region_name=region_name, endpoint_url=endpoint_url)
        else:
            self.s3_client = boto3.client('s3', region_name=region_name)
        self.bucket_name = bucket_name

    def calculate_hash(self, file_path):
        """Calculate the MD5 hash of a file."""
        hash_md5 = hashlib.md5()
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                hash_md5.update(chunk)
        return hash_md5.hexdigest()

    def upload_file(self, file_path, object_name=None):
        """Upload a file to an S3 bucket."""
        if object_name is None:
            object_name = os.path.basename(file_path)

        file_hash = self.calculate_hash(file_path)
        if self.check_file_exists(file_hash):
            print(f"File {file_path} already exists in the bucket.")
            return

        try:
            self.s3_client.upload_file(file_path, self.bucket_name, object_name)
            print(f"File {file_path} uploaded to {self.bucket_name}/{object_name}")
        except FileNotFoundError:
            print("The file was not found")
        except NoCredentialsError:
            print("Credentials not available")

    def check_file_exists(self, file_hash):
        """Check if a file with the same hash already exists in the bucket."""
        response = self.s3_client.list_objects_v2(Bucket=self.bucket_name)
        if 'Contents' in response:
            for obj in response['Contents']:
                obj_hash = self.calculate_hash(f"s3://{self.bucket_name}/{obj['Key']}")
                if obj_hash == file_hash:
                    return True
        return False

    def delete_file(self, object_name):
        """Delete a file from the S3 bucket."""
        try:
            self.s3_client.delete_object(Bucket=self.bucket_name, Key=object_name)
            print(f"File {object_name} deleted from {self.bucket_name}")
        except Exception as e:
            print(f"Error deleting file: {e}")