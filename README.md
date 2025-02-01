<h1 align="center">
  <img src="./blazing_board/assets/logo_blazing_board.png" alt="Alt Text"/>

<div align="center">
  <a href="https://blazingboard.ch/">website</a>
</div>
</h1>

# BlazingBoard

Blazing fast typing exercises online

## Setup

This project is using Google Cloud for:

  - Hosting the fullstack Dioxus web app *blazing_board*, with Google Cloud Run
  - Storing the stories that you type in *blazing_board*, with Firestore
  - Adding daily stories in *content_updater, with OpenAI and Google Cloud Scheduler

### Requirements

dioxus-cli

### Config .env for *blazing_board*

```
# For Firestore
PROJECT_ID=
DATABASE_ID=
# To tell the app that you are not running in GCP so you need to auth with a `key.json` in the subfolder
# If you are using Google cloud auth, you can comment this line
IAMTHEDEV=1
```

### Config .env for *content_updater*

```
OPENAI_API_KEY=
# For Firestore
PROJECT_ID=
DATABASE_ID=
# To tell the app that you are not running in GCP so you need to auth with a `key.json` in the subfolder
# If you are using Google cloud auth, you can comment this line
IAMTHEDEV=1
```

## Run *blazing_board* for local development

```bash
cd blazing_board
dx serve --platform web
```
