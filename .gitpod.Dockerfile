FROM ubuntu:devel

RUN apt update -y --allow-unauthenticated

RUN apt-get install libgtk-4-dev
