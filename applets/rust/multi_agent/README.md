## To deploy
```deploy -e --widl-file /home/ubuntu/weilliptic/mcp-vault/rust/multi_agent/multi_agent.widl --file-path /home/ubuntu/weilliptic/mcp-vault/rust/multi_agent/target/wasm32-unknown-unknown/release/multi_agent.wasm -i '{"description":"aurora and email agent", "agent_addresses":["aaaaaaqq7gdsyamjzj3ok4hv3fg6koii7tmksr3oioholpyxalyig44sr4", "aaaaaasl5xzqjicyqii2zcdvhshlzujyfctgobhmvb3pp335lhs2cfhhsi"]}'
```
where agent addresses can be any mcp applet address

## To execute a task
```execute -n aaaaaaxuuqzwapxx6vdln2i2z5oi74ia2oig4ry6cbqpgzdotvdahvesg4 -m run_tasks -i '{"task_descriptions":["get 5 album titles", "send an email from somya@weilliptic.com to somya@weilliptic.com with the subject as hi and body as the previous result"]}'
```