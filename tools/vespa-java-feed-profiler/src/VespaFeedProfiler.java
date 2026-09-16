import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Measures synchronous Vespa Document API acknowledgements from Java.
 *
 * The tool deliberately uses only the JDK HTTP client. A successful response
 * is treated as the feed completion point: the operation has been accepted,
 * persisted, processed, and made visible by the Vespa Document API.
 */
public final class VespaFeedProfiler {
    private VespaFeedProfiler() {}

    public static void main(String[] args) throws Exception {
        if (contains(args, "--help") || contains(args, "-h")) {
            printUsage();
            return;
        }

        Options options;
        try {
            options = Options.parse(args);
        } catch (IllegalArgumentException error) {
            System.err.println("error: " + error.getMessage());
            System.err.println("Use --help for usage.");
            System.exit(2);
            return;
        }

        int exitCode = run(options);
        System.exit(exitCode);
    }

    private static int run(Options options) throws InterruptedException {
        ExecutorService workers = Executors.newFixedThreadPool(options.concurrency);
        HttpClient client = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(options.timeoutSeconds))
                .version(HttpClient.Version.HTTP_1_1)
                .build();

        List<OperationResult> results = Collections.synchronizedList(new ArrayList<>());
        AtomicInteger next = new AtomicInteger();
        CountDownLatch ready = new CountDownLatch(options.concurrency);
        CountDownLatch start = new CountDownLatch(1);
        List<Future<?>> futures = new ArrayList<>();

        for (int worker = 0; worker < options.concurrency; worker++) {
            futures.add(workers.submit(() -> {
                ready.countDown();
                try {
                    start.await();
                    while (true) {
                        int index = next.getAndIncrement();
                        if (index >= options.count) {
                            return null;
                        }
                        results.add(send(client, options, index));
                    }
                } catch (InterruptedException error) {
                    Thread.currentThread().interrupt();
                    throw error;
                }
            }));
        }

        ready.await();
        Instant startedAt = Instant.now();
        long runStart = System.nanoTime();
        start.countDown();
        boolean workerFailure = false;
        for (Future<?> future : futures) {
            try {
                future.get();
            } catch (ExecutionException error) {
                workerFailure = true;
                System.err.println("worker failed: " + error.getCause());
            }
        }
        long runEnd = System.nanoTime();
        workers.shutdown();
        workers.awaitTermination(10, TimeUnit.SECONDS);

        results.sort(Comparator.comparingInt(result -> result.index));
        printReport(options, startedAt, runEnd - runStart, results, workerFailure);
        return workerFailure || results.size() != options.count || hasFailure(results) ? 1 : 0;
    }

    private static OperationResult send(HttpClient client, Options options, int index) {
        String documentId = options.idPrefix + "-" + index;
        URI uri = URI.create(options.endpoint + "/" + path(options.namespace)
                + "/" + path(options.documentType) + "/docid/" + path(documentId));
        String body = documentJson(options, documentId);
        HttpRequest request = HttpRequest.newBuilder(uri)
                .timeout(Duration.ofSeconds(options.timeoutSeconds))
                .header("Content-Type", "application/json")
                .PUT(HttpRequest.BodyPublishers.ofString(body))
                .build();

        long started = System.nanoTime();
        try {
            HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
            double latencyMillis = nanosToMillis(System.nanoTime() - started);
            boolean successful = response.statusCode() >= 200 && response.statusCode() < 300;
            String error = successful ? "" : "HTTP " + response.statusCode() + ": "
                    + truncate(response.body());
            return new OperationResult(index, latencyMillis, response.statusCode(), error);
        } catch (IOException error) {
            return new OperationResult(index, nanosToMillis(System.nanoTime() - started), 0,
                    error.toString());
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            return new OperationResult(index, nanosToMillis(System.nanoTime() - started), 0,
                    error.toString());
        }
    }

    private static String documentJson(Options options, String documentId) {
        StringBuilder json = new StringBuilder();
        json.append("{\"put\":\"").append(jsonEscape("id:" + options.namespace + ":"
                + options.documentType + "::" + documentId)).append("\",\"fields\":{");
        json.append("\"").append(jsonEscape(options.textField)).append("\":\"")
                .append(jsonEscape(options.text)).append("\"");
        if (options.integerField != null) {
            json.append(",\"").append(jsonEscape(options.integerField)).append("\":")
                    .append(options.integerValue);
        }
        return json.append("}}").toString();
    }

    private static void printReport(Options options, Instant startedAt, long elapsedNanos,
                                    List<OperationResult> results, boolean workerFailure) {
        List<Double> latencies = new ArrayList<>();
        for (OperationResult result : results) {
            latencies.add(result.latencyMillis);
        }
        Collections.sort(latencies);
        int acknowledged = 0;
        for (OperationResult result : results) {
            if (result.error.isEmpty()) {
                acknowledged++;
            }
        }

        double elapsedMillis = nanosToMillis(elapsedNanos);
        StringBuilder json = new StringBuilder();
        json.append("{\n")
                .append("  \"started_at\": \"").append(startedAt).append("\",\n")
                .append("  \"endpoint\": \"").append(jsonEscape(options.endpoint)).append("\",\n")
                .append("  \"namespace\": \"").append(jsonEscape(options.namespace)).append("\",\n")
                .append("  \"document_type\": \"").append(jsonEscape(options.documentType)).append("\",\n")
                .append("  \"count\": ").append(options.count).append(",\n")
                .append("  \"concurrency\": ").append(options.concurrency).append(",\n")
                .append("  \"acknowledged\": ").append(acknowledged).append(",\n")
                .append("  \"failed\": ").append(results.size() - acknowledged).append(",\n")
                .append("  \"worker_failure\": ").append(workerFailure).append(",\n")
                .append("  \"all_acknowledgments_ms\": ").append(format(elapsedMillis)).append(",\n")
                .append("  \"throughput_docs_per_second\": ")
                .append(format(options.count / (elapsedMillis / 1000.0))).append(",\n")
                .append("  \"ack_latency_ms\": {\n")
                .append("    \"min\": ").append(format(latencies.get(0))).append(",\n")
                .append("    \"average\": ").append(format(average(latencies))).append(",\n")
                .append("    \"median\": ").append(format(percentile(latencies, 0.50))).append(",\n")
                .append("    \"p95\": ").append(format(percentile(latencies, 0.95))).append(",\n")
                .append("    \"max\": ").append(format(latencies.get(latencies.size() - 1))).append("\n")
                .append("  },\n")
                .append("  \"errors\": [");
        int errorsPrinted = 0;
        for (OperationResult result : results) {
            if (!result.error.isEmpty() && errorsPrinted < 5) {
                if (errorsPrinted++ > 0) {
                    json.append(",");
                }
                json.append("\n    {\"index\": ").append(result.index)
                        .append(", \"status\": ").append(result.status)
                        .append(", \"message\": \"").append(jsonEscape(result.error)).append("\"}");
            }
        }
        if (errorsPrinted > 0) {
            json.append("\n  ");
        }
        json.append("]\n}\n");
        System.out.print(json);
    }

    private static boolean hasFailure(List<OperationResult> results) {
        for (OperationResult result : results) {
            if (!result.error.isEmpty()) {
                return true;
            }
        }
        return false;
    }

    private static double average(List<Double> values) {
        double total = 0;
        for (double value : values) {
            total += value;
        }
        return total / values.size();
    }

    private static double percentile(List<Double> values, double percentile) {
        int index = (int) Math.ceil(percentile * values.size()) - 1;
        return values.get(Math.max(0, Math.min(index, values.size() - 1)));
    }

    private static double nanosToMillis(long nanos) {
        return nanos / 1_000_000.0;
    }

    private static String format(double value) {
        return String.format(Locale.ROOT, "%.3f", value);
    }

    private static String path(String value) {
        if (!value.matches("[A-Za-z0-9_.:-]+")) {
            throw new IllegalArgumentException("path value contains unsupported characters: " + value);
        }
        return value;
    }

    private static String jsonEscape(String value) {
        StringBuilder escaped = new StringBuilder();
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            switch (character) {
                case '\\': escaped.append("\\\\"); break;
                case '"': escaped.append("\\\""); break;
                case '\b': escaped.append("\\b"); break;
                case '\f': escaped.append("\\f"); break;
                case '\n': escaped.append("\\n"); break;
                case '\r': escaped.append("\\r"); break;
                case '\t': escaped.append("\\t"); break;
                default:
                    if (character < 0x20) {
                        escaped.append(String.format(Locale.ROOT, "\\u%04x", (int) character));
                    } else {
                        escaped.append(character);
                    }
            }
        }
        return escaped.toString();
    }

    private static String truncate(String value) {
        String compact = value.replaceAll("\\s+", " ").trim();
        return compact.length() <= 200 ? compact : compact.substring(0, 200) + "...";
    }

    private static boolean contains(String[] args, String wanted) {
        for (String arg : args) {
            if (wanted.equals(arg)) {
                return true;
            }
        }
        return false;
    }

    private static void printUsage() {
        System.out.println("Usage: run.sh [options]\n"
                + "  --endpoint URL          Document API base (default http://localhost:8080/document/v1)\n"
                + "  --namespace NAME        Vespa namespace (default profile)\n"
                + "  --document-type NAME    Vespa document type (default typea)\n"
                + "  --count N               Documents to put (default 20)\n"
                + "  --concurrency N         In-flight Java requests (default 1)\n"
                + "  --text-field NAME       Text field to populate (default title)\n"
                + "  --text VALUE            Text value (default java-profile)\n"
                + "  --integer-field NAME    Optional integer field\n"
                + "  --integer-value N       Integer value (default 1)\n"
                + "  --id-prefix PREFIX      Unique document ID prefix\n"
                + "  --timeout-seconds N     Connect/request timeout (default 60)\n"
                + "  --help                  Show this help\n");
    }

    private static final class OperationResult {
        private final int index;
        private final double latencyMillis;
        private final int status;
        private final String error;

        private OperationResult(int index, double latencyMillis, int status, String error) {
            this.index = index;
            this.latencyMillis = latencyMillis;
            this.status = status;
            this.error = error;
        }
    }

    private static final class Options {
        private String endpoint = "http://localhost:8080/document/v1";
        private String namespace = "profile";
        private String documentType = "typea";
        private int count = 20;
        private int concurrency = 1;
        private String textField = "title";
        private String text = "java-profile";
        private String integerField;
        private int integerValue = 1;
        private String idPrefix = "java-profile-" + System.currentTimeMillis();
        private int timeoutSeconds = 60;

        private static Options parse(String[] args) {
            Options options = new Options();
            for (int index = 0; index < args.length; index++) {
                String argument = args[index];
                switch (argument) {
                    case "--endpoint": options.endpoint = value(args, ++index, argument); break;
                    case "--namespace": options.namespace = value(args, ++index, argument); break;
                    case "--document-type": options.documentType = value(args, ++index, argument); break;
                    case "--count": options.count = positiveInt(args, ++index, argument); break;
                    case "--concurrency": options.concurrency = positiveInt(args, ++index, argument); break;
                    case "--text-field": options.textField = value(args, ++index, argument); break;
                    case "--text": options.text = value(args, ++index, argument); break;
                    case "--integer-field": options.integerField = value(args, ++index, argument); break;
                    case "--integer-value": options.integerValue = intValue(args, ++index, argument); break;
                    case "--id-prefix": options.idPrefix = value(args, ++index, argument); break;
                    case "--timeout-seconds": options.timeoutSeconds = positiveInt(args, ++index, argument); break;
                    default: throw new IllegalArgumentException("unknown option: " + argument);
                }
            }
            path(options.namespace);
            path(options.documentType);
            path(options.idPrefix);
            if (!options.endpoint.startsWith("http://") && !options.endpoint.startsWith("https://")) {
                throw new IllegalArgumentException("endpoint must start with http:// or https://");
            }
            options.endpoint = trimTrailingSlash(options.endpoint);
            return options;
        }

        private static String value(String[] args, int index, String option) {
            if (index >= args.length) {
                throw new IllegalArgumentException("missing value for " + option);
            }
            return args[index];
        }

        private static int positiveInt(String[] args, int index, String option) {
            int value = intValue(args, index, option);
            if (value <= 0) {
                throw new IllegalArgumentException(option + " must be positive");
            }
            return value;
        }

        private static int intValue(String[] args, int index, String option) {
            String value = value(args, index, option);
            try {
                return Integer.parseInt(value);
            } catch (NumberFormatException error) {
                throw new IllegalArgumentException(option + " must be an integer: " + value);
            }
        }

        private static String trimTrailingSlash(String value) {
            int end = value.length();
            while (end > 0 && value.charAt(end - 1) == '/') {
                end--;
            }
            return value.substring(0, end);
        }
    }
}
