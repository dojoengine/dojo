# Katana Monitoring

This directory contains an example of how to set up monitoring for Katana using Prometheus and Grafana.

The `docker-compose.yml` file defines services for:
- Katana node
- Prometheus
- Grafana

Configuration files are provided for Prometheus and Grafana dashboards.

To run:

```
docker-compose up
```

This will start Katana with metrics enabled, Prometheus scraping those metrics, and Grafana dashboards to visualize the data.

Access the Grafana dashboard at http://localhost:3000.
