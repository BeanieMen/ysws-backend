# generic rust backend
# disclaimer the frontend is not part of the novelty project. its just there for demo purposes to all reviewers. please dont call me out on a slop frontend because this project is meant to be a backend

## prerequisites
- docker compose
- rust
- rustup
- and all the api keys listend in .env.example

## about
this is a generic rust backend for any ysws that might need a fast, scaleable backend for their event (yes scalable, ill get into that later on)

this backend includes airtable connection, hackclub oauth, hackatime oauth and other stuff i could use without actually applying for an actual ysws program

i made this to in the future use for my own ysws event hopefully wish me luck ig vro


## tech stack
i used rust, axum for api requests for the backend thats IT. it only calls api what did you even expect. this is all encompassed in a docker container along with postgres for db in another container and redis in another container for db prot. 

the backend container can be spawned n number of times. they sit behind a nginx gateway which routes requests to the available container to make it scaleable


## how this was made
it was me fighting against opencode's questionable code. i am not opencode maxxing now crodie no way in hell. i had to enable so many scopes in my slack oauth and kept forgetting. so my memory was the biggest challenge to this project and also the email only works for my email because resend doesnt allow me to send emails to other than my own with no domain

## demo links
http://beanoni.xyz:3000

## instructions to run locally
```sh
docker compose up -d --build
cd frontend
bun i
bun dev
```
 simple right. the configuration for the api keys is hell

# disclosure
ai was used in the making of this project
