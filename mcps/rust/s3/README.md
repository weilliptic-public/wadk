# AWS S3

## Core Tools

1. Upload objects (`upload_external_url_to_s3`)
2. Download Record (`download`)
3. List objects (`list_objects`)
4. Delete Record (`delete`)
5. List buckets (`list_buckets`)
6. Create Bucket (`create_bucket`)
7. Get bucket location (`get_bucket_location`)
8. Get bucket ACL (`get_bucket_acl`)
9. Get/Set bucket versioning (`get_bucket_versioning` , `set_bucket_versioning`)

## Testing

```
Weilliptic$$$> deploy --widl-file <path to>/s3.widl --file-path <path to>/s3.wasm --config-file <path to>/config.yaml 
```

## Prompt examples

- Show me all the objects in my S3 bucket 'bucketweil'
- Show me all the buckets in my S3 account
- can u upload an external file with url as https://raw.githubusercontent.com/yavuzceliker/sample-images/main/images/image-1.jpg in the bucket named bucketweil and with key uploads/mcp2.jpg
- Download the content in example.txt in the bucketweil bucket of my s3 and show me its content
- List all the buckets in my s3
- show me the location of the bucket bucketweil
