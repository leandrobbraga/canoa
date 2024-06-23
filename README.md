# Canoa

Canoa is a minimal-dependency Jira Terminal User Interface (TUI).

## Authorization

The application utilizes [Basic Auth](https://developer.atlassian.com/cloud/jira/platform/basic-auth-for-rest-apis/)
for API authorization.
To generate an API token, please refer to [this documentation](https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/).

## Configuration

Configuration can be done through environment variables or a `.env` file.

### Current Configuration Parameters:

| Variable       | Description                                           | Required |
| -------------- | ----------------------------------------------------- | -------- |
| JIRA_BOARD_ID  | Unique identifier for the board (e.g., 1234)          | Yes      |
| JIRA_HOST      | Jira's HTTP address (e.g., https://atlassian.com/)    | Yes      |
| JIRA_TOKEN     | Authorization token                                   | Yes      |
| JIRA_USER      | Username of the user (e.g., example@example.com)      | Yes      |
