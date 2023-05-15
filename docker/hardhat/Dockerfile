FROM node:16-alpine3.15

RUN apk update && apk add python3 make g++

RUN mkdir hardhat
WORKDIR hardhat
RUN npm init -y
RUN npm i hardhat
COPY hardhat.config.js .
