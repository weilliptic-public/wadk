package com.weilliptic.weilwallet.examples;

import com.weilliptic.weilwallet.agents.WeilAgent;
import com.weilliptic.weilwallet.transaction.TransactionResult;

import java.io.IOException;

/**
 * Example: wrap an agent with Weil identity and record an audit log.
 *
 * Run with: mvn exec:java -Dexec.mainClass="com.weilliptic.weilwallet.examples.WeilAgentAuditExample"
 */
public class WeilAgentAuditExample {

    /** A minimal "agent" that we wrap with Weil identity. */
    static class MyAgent {
        String process(String input) {
            return "Processed: " + input;
        }
    }

    public static void main(String[] args) throws IOException, InterruptedException {

        WeilAgent<MyAgent> weilAgent = new WeilAgent<>(new MyAgent(), "./private_key.wc");

        
        MyAgent agent = weilAgent.getAgent();
        String out = agent.process("hello");
        System.out.println(out);

        TransactionResult result = weilAgent.audit("Ran agent from java!");
        System.out.println("Audit recorded: " + result.getStatus() + " batch_id=" + result.getBatchId());
    }
}
