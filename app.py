import os
import yaml
import argparse
from flask import Flask, request, jsonify
from flask_apispec import use_kwargs, marshal_with, doc
from flask_apispec.views import MethodResource
from apispec import APISpec
from apispec.ext.marshmallow import MarshmallowPlugin
from flask_apispec.extension import FlaskApiSpec
from file_manager import FileManager
from folder_importer import FolderImporter
from tag_manager import TagManager
from marshmallow import Schema, fields

# 解析命令行参数
parser = argparse.ArgumentParser(description='File Manager API')
parser.add_argument('--config', type=str, default='config.yml', help='Path to the configuration file')
args = parser.parse_args()

# 读取配置文件
with open(args.config, 'r') as file:
    config = yaml.safe_load(file)

app = Flask(__name__)

# 初始化FileManager, FolderImporter, TagManager
if config['USE_MINIO']:
    file_manager = FileManager(
        config['S3']['BUCKET_NAME'],
        config['S3']['REGION_NAME'],
        endpoint_url=config['S3']['ENDPOINT_URL']
    )
else:
    file_manager = FileManager(
        config['S3']['BUCKET_NAME'],
        config['S3']['REGION_NAME']
    )

tag_manager = TagManager()
folder_importer = FolderImporter(file_manager, tag_manager)

# 设置Flask-APISpec
app.config.update({
    'APISPEC_SPEC': APISpec(
        title='File Manager API',
        version='v1',
        plugins=[MarshmallowPlugin()],
        openapi_version='2.0.0'
    ),
    'APISPEC_SWAGGER_URL': '/swagger/',  # URI to access API Doc JSON 
    'APISPEC_SWAGGER_UI_URL': '/swagger-ui/'  # URI to access UI of API Doc
})
docs = FlaskApiSpec(app)

# 定义请求和响应模式
class FileUploadSchema(Schema):
    file = fields.Raw(required=True)

class FileResponseSchema(Schema):
    message = fields.Str(required=True)

class TagRequestSchema(Schema):
    file_path = fields.Str(required=True)
    tags = fields.List(fields.Str(), required=True)

class TagResponseSchema(Schema):
    tags = fields.List(fields.Str(), required=True)

class FileContentResponseSchema(Schema):
    content = fields.Str(required=True)

class FolderImportSchema(Schema):
    folder_path = fields.Str(required=True)

@app.route('/upload', methods=['POST'])
@use_kwargs(FileUploadSchema, location='files')
@marshal_with(FileResponseSchema)
@doc(description='Upload a file to the object storage.')
def upload_file(file):
    """Upload a file to the object storage."""
    file_path = os.path.join('/tmp', file.filename)
    file.save(file_path)
    file_manager.upload_file(file_path)
    os.remove(file_path)
    return {'message': 'File uploaded successfully'}, 200

@app.route('/tags/<path:file_path>', methods=['GET'])
@marshal_with(TagResponseSchema)
@doc(description='Get tags for a file.')
def get_tags(file_path):
    """Get tags for a file."""
    tags = tag_manager.get_tags(file_path)
    return {'tags': tags}, 200

@app.route('/files/<path:object_name>', methods=['GET'])
@marshal_with(FileContentResponseSchema)
@doc(description='Get a file from the object storage.')
def get_file(object_name):
    """Get a file from the object storage."""
    try:
        response = file_manager.s3_client.get_object(Bucket=file_manager.bucket_name, Key=object_name)
        file_content = response['Body'].read().decode('utf-8')
        return {'content': file_content}, 200
    except Exception as e:
        return {'error': str(e)}, 500

@app.route('/folders/import', methods=['POST'])
@use_kwargs(FolderImportSchema, location='form')
@marshal_with(FileResponseSchema)
@doc(description='Import a folder into the object storage.')
def import_folder(folder_path):
    """Import a folder into the object storage."""
    folder_importer.import_folder(folder_path)
    return {'message': 'Folder imported successfully'}, 200

@app.route('/tags', methods=['POST'])
@use_kwargs(TagRequestSchema, location='form')
@marshal_with(FileResponseSchema)
@doc(description='Add tags to a file.')
def add_tags(file_path, tags):
    """Add tags to a file."""
    tag_manager.add_tags(file_path, tags)
    return {'message': 'Tags added successfully'}, 200

@app.route('/tags', methods=['DELETE'])
@use_kwargs(TagRequestSchema, location='form')
@marshal_with(FileResponseSchema)
@doc(description='Remove tags from a file.')
def remove_tags(file_path, tags):
    """Remove tags from a file."""
    tag_manager.remove_tags(file_path, tags)
    return {'message': 'Tags removed successfully'}, 200

docs.register(upload_file)
docs.register(get_tags)
docs.register(get_file)
docs.register(import_folder)
docs.register(add_tags)
docs.register(remove_tags)

if __name__ == '__main__':
    app.run(port=config['DEFAULT']['PORT'], debug=True)