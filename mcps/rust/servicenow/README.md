# ServiceNow MCP Server

ServiceNow is a cloud-based platform that automates and manages digital workflows across an organization, specializing in areas like IT, HR, and customer service.

The ServiceNow Model Context Protocol (MCP) applet integrates with the ServinceNow API to allow using natural language to handle these workflows.


```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SERVICENOW ECOSYSTEM                               │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                                CORE PLATFORM                                    │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │   USER MGMT     │    │   GROUP MGMT    │    │   UI POLICIES   │              │
│  │                 │    │                 │    │                 │              │
│  │ • Users         │    │ • Groups        │    │ • UI Policies   │              │
│  │ • Roles         │    │ • Members       │    │ • UI Actions    │              │
│  │ • Permissions   │    │ • Permissions   │    │ • Field Control │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SERVICE MANAGEMENT                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │   INCIDENTS     │    │  SERVICE CATALOG│    │  KNOWLEDGE BASE │              │
│  │                 │    │                 │    │                 │              │
│  │ • Create        │    │ • Catalog Items │    │ • Articles      │              │
│  │ • Update        │    │ • Categories    │    │ • Categories    │              │
│  │ • Resolve       │    │ • Variables     │    │ • Publishing    │              │
│  │ • Comments      │    │ • Ordering      │    │ • Search        │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                              CHANGE MANAGEMENT                                  │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │ CHANGE REQUESTS │    │  CHANGE TASKS   │    │  APPROVAL FLOW  │              │
│  │                 │    │                 │    │                 │              │
│  │ • Create        │    │ • Add Tasks     │    │ • Submit        │              │
│  │ • Risk/Impact   │    │ • Assign Users  │    │ • Approve       │              │
│  │ • Planning      │    │ • Track Progress│    │ • Reject        │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                               AGILE/SCRUM                                       │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │     EPICS       │    │     STORIES     │    │  SCRUM TASKS    │              │
│  │                 │    │                 │    │                 │              │
│  │ • High-level    │    │ • User Stories  │    │ • Task Details  │              │
│  │ • Strategic     │    │ • Story Points  │    │ • Assignments   │              │
│  │ • Long-term     │    │ • Dependencies  │    │ • Time Tracking │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                              PROJECT MANAGEMENT                                 │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │    PROJECTS     │    │   TIMELINE      │    │   RESOURCES     │              │
│  │                 │    │                 │    │                 │              │
│  │ • Project Info  │    │ • Start/End     │    │ • Project Mgrs  │              │
│  │ • Descriptions  │    │ • Milestones    │    │ • Team Members  │              │
│  │ • Status        │    │ • Dependencies  │    │ • Budget        │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                              DEVELOPMENT & DEPLOYMENT                           │
├─────────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐              │
│  │  SCRIPT INCLUDES│    │   CHANGESETS    │    │   WORKFLOWS     │              │
│  │                 │    │                 │    │                 │              │
│  │ • Server Scripts│    │ • Code Changes  │    │ • Process Flow  │              │
│  │ • Client Scripts│    │ • File Tracking │    │ • Automation    │              │
│  │ • API Functions │    │ • Commit/Publish│    │ • Notifications │              │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────┐
│                              DATA FLOW & RELATIONSHIPS                          │
└─────────────────────────────────────────────────────────────────────────────────┘

```

## Core tools


## Testing 

### Deployment
```
deploy -f <path to>/servicenow.wasm -p <path to>/servicenow.widl -c <path to>/config.yaml
```

#### `config.yaml`
```yaml
base_url: https://dev281572.service-now.com
username: <USER> 
password: <PASSWD> 
```

### Prompt examples

- Create a new incident in ServiceNow with the title "Server Down - Database Connection Issues" and description "The main database server is experiencing connection timeouts, affecting multiple applications. Users are unable to access critical business systems." Set the priority to 5.
- can u show 10 incidents whose priorities are 1?
- Add a comment to the incident with system id as "46b66a40a9fe198101f243dfbc79033d" created as "Initial investigation shows network connectivity issues between application servers and database cluster. Checking network logs and database status."
- Create a service catalog category called "IT Services" with description "Core IT services and support offerings for the organization."
- list 10 catalogs that are active
- Create a change request for "Database Server Upgrade" with description "Upgrade the main database server to improve performance and security. This includes OS updates, database version upgrade, and security patches." Set priority to "2 - High" and risk level to "Medium".
- list 2 change requests whose state is 3 and risk is 2.
- Add a change task to the change request with id as "46e9b4afa9fe198101026e122b85f442" called "Backup Current Database" with description "Create full backup of all databases before proceeding with upgrade".
- Submit the change request with id "46e9b4afa9fe198101026e122b85f442" for approval.
- Create a new user story called "Implement User Authentication" with description "As a user, I want to securely log into the system so that my data is protected" and set story points to 8.
- Create an epic called "Security Enhancement Project" with description "Comprehensive security improvements across all systems including authentication, authorization, and data protection measures."
- Create a scrum task called "Design Authentication UI" with description "Create wireframes and mockups for the new authentication interface" and link it to the story with id "2881e3f9c3ff2210ddc579ec050131f3"
- Create a project called "Digital Transformation Initiative" with description "Modernize legacy systems and implement new digital capabilities to improve business efficiency" and goal as "bring a disruptive change"
- Create a workflow called "Incident Escalation Process" for the incident table with description "Automatically escalate incidents based on priority and time open."
- Create a script include called "DataValidationUtils" with description "Utility functions for validating data across different tables" and add a function that validates email addresses.
- Create a changeset called "Authentication Module Updates" with description "Changes related to the new authentication system including UI updates and backend logic."
- Commit the Authentication Module Updates changeset having id "4c8852e2c3772610ddc579ec05013124"
- Create a knowledge base called "IT Support Knowledge Base" with description "Central repository for IT support documentation and troubleshooting guides."
- Create an article called "How to Reset Password" with content "Step-by-step guide for users to reset their passwords: 1. Go to the login page 2. Click 'Forgot Password' 3. Enter your email address 4. Check your email for reset link 5. Follow the instructions in the email". use the knowledge base sys id as "236a8222c3372610ddc579ec050131fd"
- publish the article with sys_id "4dca2bfac3332210ddc579ec050131c5"
- Create a new user called "John Smith" with username "john.smith", email "john.smith@company.com", first name "John", last name "Smith", and assign to the IT department.
- Create a user information of user whose identifier is "John Smith" 
- Create a group called "Database Administrators" with description "Team responsible for database management and maintenance" 
- Create a UI policy action for the policy with sys_id as "014a1e26c3772610ddc579ec05013168" that shows the resolution notes field only when the incident state is "Resolved" or "Closed".
- List all incidents that are currently open and have priority 1 or 2.
- List all change requests that are currently in approval status.
- List all user stories in the epic with id "c957c2aac3f32610ddc579ec050131b1"
- List all projects that are currently active and managed by the IT director.
- List all workflows that are active and related to incident management.
- List the name of any 1 script includes which is active
- List all changesets that are ready for deployment.

