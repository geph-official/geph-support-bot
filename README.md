# geph-support-bot

GephSupportBot is an AI support bot developed for [Geph](https://geph.io/). It currently supports Telegram and Email, and enables field-programming by an admin via Telegram.

The bot can be configured by the `config.yaml` file. The fields of the config file are:

```yaml
history_db: absolute path to where to put the bot's conversations database

llm_config:
  openai_key: OpenAI API key
  # both main_model and fallback_model must be specified in OpenAI's API format
  main_model: main model used for the bot (gpt-4 recommended)
  fallback_model: optional field. Falls back to this model 
  if the main_model doesn't reply in 5 minutes (gpt-3.5-turbo recommended)

# to disable telegram support, comment out the entire telegram_config block
telegram_config:
  telegram_token: the telegram bot's secret token
  admin_uname: the bot admin's telegram username, without @
  bot_uname: username of the bot

# to disable email support, comment out the entire email_config block
email_config: 
  mailgun_url: email sending url for Mailgun
  mailgun_key: Mailgun API key
  address: email address to send emails from with Mailgun
  signature: signature placed at the end of emails
  cc: optional field. Email address to forward all emails to

# to disable actions, comment out the entire actions_config block
actions_config:
    # this block is for putting all info needed for actions 
    # to be performed by the bot. You can add actions by
    # editing the source code.
```

To run GephSupportBot:
1. Create a `config.yaml` with all the fields you need
2. Edit the `initial-prompt.txt` file to have a prompt suitable for your usecase. This will be fed to OpenAI as the system prompt.
3. Compile `geph-support-bot` for your platform and run with `cargo run -- -c [path/to/your/config.yaml]`



## Telegram
GephSupportBot supports both group chat and private messaging on Telegram. The bot responds to all private messages.

In group chats, it will respond to all messages containing `@[bot_username]`, and all messages that respond to a message from itself. In responding, it takes into account previous conversations mentioning itself in the same group chat, as far back as space would allow. 


To set up GephSupportBot as a Telegram bot: 
1. First, [create a Telegram bot with `@BotFather`](https://www.freecodecamp.org/news/how-to-create-a-telegram-bot-using-python/#:~:text=Type%20%2Fnewbot%20%2C%20and%20follow%20the,access%20to%20the%20Telegram%20API.&text=Note%3A%20Make%20sure%20you%20store,can%20easily%20manipulate%20your%20bot.)
2. Then, populate the `telegram_config` block in your `config.yaml`

The bot can be field-programmed by the `admin` specified in `config.yaml` to learn facts using the `#learn` keyword. To do so, the `admin` can simply type `@[bot_username] #learn [what the bot should learn]` in a group chat or simply `#learn [what the bot should learn]` in a private message to the bot. The bot will then reply with what it has learned; this is usually a concise summary of the admin's `#learn` message.


## Email
GephSupportBot currently supports sending and receiving emails using [Mailgun](https://www.mailgun.com/). 

To enable email support, you need:
1. A working Mailgun account
2. A server with a public-facing IP address
3. A dedicated domain for receiving emails (for example: `bot.geph.io`)
4. A DNS provider (such as Cloudflare)

Once you've gathered the required pieces, set up the bot with the following steps:
1. Set up GephSupportBot on your server per the steps outlined earlier
2. Fill in the `email_config` block of GephSupportBot's `config.yaml`
4. Add your domain to Mailgun and set up Mailgun as the mail server for your domain: see [this tutorial](https://help.mailgun.com/hc/en-us/articles/203637190-How-Do-I-Add-or-Delete-a-Domain-)
5. Set up a Mailgun route for receiving emails and forwarding them to GephSupportBot. If you want to forward all the received emails to another email address to make monitoring the bot easier, add that address to the route as well. See [this tutorial](https://help.mailgun.com/hc/en-us/articles/360011355893-How-Do-I-Setup-a-Route-#:~:text=First%2C%20log%20in%20to%20the,right%20portion%20of%20the%20page.).
6. Test that everything works!

## Adding support for new platforms
We welcome contributions for extending GephSupportBot to other platforms!

## Actions
It is possible to program GephSupportBot to perform actions (like modifying entries in a database) when the selected LLM deems fit, according to a prompt. You can refer to our example in `actions.rs` for how to do this.
