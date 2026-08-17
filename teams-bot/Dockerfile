FROM python:3.12-slim

LABEL org.opencontainers.image.title="Neural Agent OS - Teams Meeting Bot"
LABEL org.opencontainers.image.description="Joins scheduled Teams meetings, captures participant audio, and transfers encrypted data to the local Neural application."

RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY bot.py .

RUN mkdir -p /data/recordings /data/metadata
VOLUME ["/data"]

ENV NEURAL_TEAMS_DATA_DIR=/data
ENV NEURAL_API_URL=http://host.docker.internal:8787
# REQUIRED: shared secret (must match NAO_TEAMS_BOT_SECRET on the local app)
# ENV NAO_TEAMS_BOT_SECRET=change-me

EXPOSE 8445

HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD python -c "import aiohttp; import asyncio; asyncio.run(aiohttp.ClientSession().get('${NEURAL_API_URL}/health'))" || exit 1

ENTRYPOINT ["python", "-u", "bot.py"]
