FROM postgres:17 AS env-build

# install build dependencies
RUN apt-get update && apt-get -y upgrade \
  && apt-get install -y build-essential libpq-dev postgresql-server-dev-all git

WORKDIR /srv

# Clone the pg_uuidv7 repository
RUN git clone https://github.com/fboulnois/pg_uuidv7.git .

# build extension only for PostgreSQL 17
RUN pg_buildext build-17 17

# create tarball and checksums
RUN cp sql/pg_uuidv7--1.6.sql . && TARGETS=$(find * -name pg_uuidv7.so) \
  && tar -czvf pg_uuidv7.tar.gz $TARGETS pg_uuidv7--1.6.sql pg_uuidv7.control \
  && sha256sum pg_uuidv7.tar.gz $TARGETS pg_uuidv7--1.6.sql pg_uuidv7.control > SHA256SUMS

FROM postgres:17 AS env-deploy

# copy tarball and checksums
COPY --from=0 /srv/pg_uuidv7.tar.gz /srv/SHA256SUMS /srv/

# add extension to postgres
COPY --from=0 /srv/17/pg_uuidv7.so /usr/lib/postgresql/17/lib
COPY --from=0 /srv/pg_uuidv7.control /usr/share/postgresql/17/extension
COPY --from=0 /srv/pg_uuidv7--1.6.sql /usr/share/postgresql/17/extension