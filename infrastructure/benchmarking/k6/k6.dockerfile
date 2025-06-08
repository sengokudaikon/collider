# k6 Load Testing Docker Image
# Provides a containerized environment for running k6 tests

FROM grafana/k6:latest

# Install additional tools for reporting
USER root
RUN apk add --no-cache \
    curl \
    jq \
    bash \
    ca-certificates

# Create directories for tests and results
WORKDIR /tests
RUN mkdir -p /tests/scenarios /tests/results

# Copy test scripts
COPY *.js /tests/
COPY scenarios/ /tests/scenarios/
COPY *.sh /tests/

# Make scripts executable
RUN chmod +x /tests/*.sh

# Set default user back to k6
USER k6

# Default command
CMD ["run", "/tests/load-test.js"]

# Labels
LABEL maintainer="collider-team"
LABEL version="1.0"
LABEL description="k6 load testing environment for Collider"

# Environment variables
ENV K6_OUT=json
ENV K6_SUMMARY_EXPORT=/tests/results/summary.json