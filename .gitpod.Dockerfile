FROM ubuntu:devel

RUN apt-get install gpgv
RUN apt-get install gpgv2

RUN apt-get update

RUN apt-get install libgtk-4-dev
