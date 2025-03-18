///*
// * MIT License
// * ... (license header)
// */
//
//package github.hua0512.flv.network
//
//import github.hua0512.flv.data.FlvHeader
//import github.hua0512.flv.data.FlvTag
//import github.hua0512.flv.operators.NetworkCondition
//import github.hua0512.flv.utils.isAudioTag
//import github.hua0512.flv.utils.isVideoTag
//import github.hua0512.utils.logger
//import kotlinx.coroutines.CoroutineScope
//import kotlinx.coroutines.Dispatchers
//import kotlinx.coroutines.flow.MutableStateFlow
//import kotlinx.coroutines.flow.StateFlow
//import kotlinx.coroutines.flow.asStateFlow
//import kotlinx.coroutines.launch
//import kotlinx.io.Source
//import java.time.Duration
//import java.time.Instant
//import java.util.*
//import kotlin.math.min
//
//private const val TAG = "SourceNetworkMonitor"
//private val logger = logger(TAG)
//
///**
// * Network monitor specifically designed for FLV parsing operations
// */
//class NetworkMonitor(
//  private val scope: CoroutineScope,
//  private val sampleWindowSize: Int = 100,
//  private val degradationThreshold: Int = 5,
//  private val readTimeout: Duration = Duration.ofSeconds(30), // Increased to match WAIT_FOR_DATA_TIMEOUT
//) {
//  private val _networkCondition = MutableStateFlow(NetworkCondition.MODERATE)
//  val networkCondition: StateFlow<NetworkCondition> = _networkCondition.asStateFlow()
//
//  private class CircularBuffer<T>(private val maxSize: Int) {
//    private val buffer = ArrayDeque<Pair<Instant, T>>(maxSize)
//
//
//    val size get() = buffer.size
//
//    fun add(timestamp: Instant, value: T) {
//      if (buffer.size >= maxSize) {
//        buffer.removeFirst()
//      }
//      buffer.addLast(timestamp to value)
//    }
//
//    fun removeOlderThan(instant: Instant) {
//      while (buffer.isNotEmpty() && buffer.first().first.isBefore(instant)) {
//        buffer.removeFirst()
//      }
//    }
//
//    fun removeFirst() {
//      buffer.removeFirst()
//    }
//
//    val values: List<T>
//      get() = buffer.map { it.second }
//
//    fun isEmpty() = buffer.isEmpty()
//    fun clear() = buffer.clear()
//
//    fun removeIf(predicate: (Instant) -> Boolean) {
//      buffer.removeIf { predicate(it.first) }
//    }
//  }
//
//  private val stats = CircularBuffer<NetworkStats>(sampleWindowSize)
//  private var lastReadTime: Instant = Instant.now()
//  private var baselineReadTime: Long = 0
//  private var totalBytesRead: Long = 0
//  private var lastBytesRead: Long = 0
//  private var readRate: Double = 0.0 // bytes per second
//
//  private var lastTagTimestamp: Int = 0
//  private val timestampGaps = CircularBuffer<Int>(sampleWindowSize)
//  private var tagCount: Int = 0
//
//  private val bytesReadWindow = ArrayDeque<Pair<Instant, Long>>(50) // Pre-sized capacity
//  private val readRateWindowDuration = Duration.ofSeconds(5)
//
//  private var lastConditionUpdate = Instant.now()
//  private val conditionUpdateInterval = Duration.ofMillis(500) // Update at most every 500ms
//
//  init {
//    scope.launch(Dispatchers.Default) {
//      cleanupOldStats()
//    }
//  }
//
//  /**
//   * Record successful header read
//   */
//  suspend fun recordHeaderRead(header: FlvHeader, source: Source) {
//    val now = Instant.now()
//    if (baselineReadTime == 0L) {
//      baselineReadTime = now.toEpochMilli()
//    }
//
//    updateStats(
//      NetworkStats(
//        packetLoss = 0.0,
//        latency = calculateReadLatency(source),
//        jitter = 0.0,
//        consecutiveErrors = 0,
//        timestamp = now,
//        bytesRead = header.headerSize.toLong(),
//        readRate = readRate,
//        isHeader = true
//      )
//    )
//
//    lastReadTime = now
//    lastBytesRead = header.headerSize.toLong()
//    updateNetworkCondition()
//  }
//
//  /**
//   * Record successful tag read
//   */
//  suspend fun recordTagRead(tag: FlvTag, source: Source) {
//    val now = Instant.now()
//    tagCount++
//
//    // Calculate timestamp gap for video/audio tags
//    if (tag.isVideoTag() || tag.isAudioTag()) {
//      val timestampGap = if (lastTagTimestamp > 0) {
//        tag.header.timestamp - lastTagTimestamp
//      } else 0
//      timestampGaps.add(now, timestampGap)
//      if (timestampGaps.size > sampleWindowSize) {
//        timestampGaps.removeFirst()
//      }
//      lastTagTimestamp = tag.header.timestamp
//    }
//
//    totalBytesRead += tag.size
//    updateReadRate(now)
//
//    val readLatency = calculateReadLatency(source)
//    val jitter = calculateTagJitter()
//
//    updateStats(
//      NetworkStats(
//        packetLoss = 0.0,
//        latency = readLatency,
//        jitter = jitter,
//        consecutiveErrors = 0,
//        timestamp = now,
//        bytesRead = tag.size,
//        readRate = readRate,
//        tagTimestamp = tag.header.timestamp
//      )
//    )
//
//    lastReadTime = now
//    lastBytesRead = tag.size
//    updateNetworkCondition()
//  }
//
//  /**
//   * Calculate jitter based on tag timestamps
//   */
//  private fun calculateTagJitter(): Double {
//    if (timestampGaps.size < 2) return 0.0
//
//    // Calculate standard deviation of timestamp gaps
//    val mean = timestampGaps.values.average()
//    val variance = timestampGaps.values.map { (it - mean) * (it - mean) }.average()
//    return kotlin.math.sqrt(variance)
//  }
//
//  /**
//   * Enhanced network stats for FLV operations
//   */
//  data class NetworkStats(
//    val packetLoss: Double = 0.0,
//    val latency: Long = 0,
//    val jitter: Double = 0.0,
//    val consecutiveErrors: Int = 0,
//    val timestamp: Instant = Instant.now(),
//    val bytesRead: Long = 0,
//    val readRate: Double = 0.0,
//    val isHeader: Boolean = false,
//    val tagTimestamp: Int = 0,
//  )
//
//  /**
//   * Update network condition based on FLV-specific metrics
//   */
//  private fun updateNetworkCondition() {
//    val now = Instant.now()
//    if (Duration.between(lastConditionUpdate, now) < conditionUpdateInterval) {
//      return
//    }
//
//    val currentStats = getAggregatedStats()
//    val newCondition = determineNetworkCondition(currentStats)
//
//    if (newCondition != _networkCondition.value) {
//      _networkCondition.value = newCondition
//      if (logger.isInfoEnabled) {
//        logNetworkConditionChange(newCondition, currentStats)
//      }
//    }
//
//    lastConditionUpdate = now
//  }
//
//  private fun determineNetworkCondition(stats: NetworkStats): NetworkCondition = when {
//    isPoorCondition(stats) -> NetworkCondition.POOR
//    isGoodCondition(stats) -> NetworkCondition.GOOD
//    else -> NetworkCondition.MODERATE
//  }
//
//  /**
//   * Check for abnormal gaps in tag timestamps
//   */
//  private fun hasAbnormalTimestampGaps(): Boolean {
//    val gaps = timestampGaps.values
//    if (gaps.size < 2) return false
//
//    // Calculate median more efficiently
//    val sorted = gaps.sorted()
//    val median = sorted[sorted.size / 2]
//    val threshold = median * 3
//
//    // Use any with a predicate instead of creating new list
//    return gaps.any { it > threshold }
//  }
//
//  /**
//   * Calculate read latency from Source
//   */
//  private fun calculateReadLatency(source: Source): Long {
//    return readTimeout.toMillis()
//  }
//
//  /**
//   * Update read rate calculation
//   */
//  private fun updateReadRate(now: Instant) {
//    // Optimize window updates
//    while (bytesReadWindow.isNotEmpty() &&
//      bytesReadWindow.first().first.isBefore(now.minus(readRateWindowDuration))
//    ) {
//      bytesReadWindow.removeFirst()
//    }
//
//    bytesReadWindow.addLast(now to lastBytesRead)
//
//    if (bytesReadWindow.size >= 2) {
//      val firstReading = bytesReadWindow.first()
//      val lastReading = bytesReadWindow.last()
//      val windowDuration = Duration.between(firstReading.first, lastReading.first)
//
//      if (!windowDuration.isZero) {
//        // Use fold instead of sumOf for better performance
//        val windowBytes = bytesReadWindow.fold(0L) { acc, pair -> acc + pair.second }
//        val windowRate = windowBytes.toDouble() / windowDuration.toMillis() * 1000
//
//        readRate = (readRate * 0.8) + (windowRate * 0.2)
//
//        if (logger.isDebugEnabled) {
//          logger.trace(
//            "Read rate: {} bytes/s (window: {}ms)",
//            String.format("%.2f", readRate),
//            windowDuration.toMillis()
//          )
//        }
//      }
//    }
//  }
//
//  /**
//   * Check if Source reading is stalled
//   */
//  fun isReadStalled(): Boolean {
//    val timeSinceLastRead = Duration.between(lastReadTime, Instant.now())
//    return timeSinceLastRead > readTimeout
//  }
//
//  /**
//   * Get current read rate in bytes per second
//   */
//  fun getCurrentReadRate(): Double = readRate
//
//  /**
//   * Reset the monitor
//   */
//  fun reset() {
//    stats.clear()
//    bytesReadWindow.clear()
//    timestampGaps.clear()
//    lastReadTime = Instant.now()
//    lastConditionUpdate = Instant.now()
//    baselineReadTime = 0
//    totalBytesRead = 0
//    lastBytesRead = 0
//    readRate = 0.0
//    _networkCondition.value = NetworkCondition.MODERATE
//    lastTagTimestamp = 0
//    tagCount = 0
//  }
//
//  /**
//   * Record read error from Source
//   */
//  suspend fun recordReadError(source: Source, error: Throwable? = null) {
//    val now = Instant.now()
//    val lastStats = stats.values.lastOrNull()
//    val consecutiveErrors = (lastStats?.consecutiveErrors ?: 0) + 1
//
//    updateStats(
//      NetworkStats(
//        packetLoss = calculatePacketLoss(),
//        latency = calculateReadLatency(source),
//        jitter = lastStats?.jitter ?: 0.0,
//        consecutiveErrors = consecutiveErrors,
//        timestamp = now,
//        bytesRead = 0,
//        readRate = 0.0
//      )
//    )
//
//    logger.warn("$TAG Source read error: ${error?.message}, consecutive errors: $consecutiveErrors")
//    updateNetworkCondition()
//  }
//
//  /**
//   * Calculate current packet loss rate
//   */
//  private fun calculatePacketLoss(): Double {
//    if (stats.isEmpty()) return 0.0
//    val errorCount = stats.values.count { it.consecutiveErrors > 0 }
//    return (errorCount.toDouble() / min(stats.size, sampleWindowSize)) * 100
//  }
//
//  /**
//   * Get aggregated statistics over the sample window
//   */
//  private fun getAggregatedStats(): NetworkStats {
//    if (stats.isEmpty()) return NetworkStats()
//
//    // Get recent stats using a list and subList
//    val allStats = stats.values.toList()
//    val startIndex = maxOf(0, allStats.size - sampleWindowSize)
//    val recentStats = allStats.subList(startIndex, allStats.size)
//
//    return NetworkStats(
//      packetLoss = calculatePacketLoss(),
//      latency = recentStats.map { it.latency }.average().toLong(),
//      jitter = recentStats.map { it.jitter }.average(),
//      consecutiveErrors = recentStats.maxOf { it.consecutiveErrors },
//      bytesRead = recentStats.sumOf { it.bytesRead },
//      readRate = readRate
//    )
//  }
//
//  /**
//   * Update stats collection with new data
//   */
//  private suspend fun updateStats(newStats: NetworkStats) {
//    stats.add(newStats.timestamp, newStats)
//    cleanupOldStats()
//  }
//
//  /**
//   * Clean up old statistics
//   */
//  private suspend fun cleanupOldStats() {
//    val cutoffTime = Instant.now().minus(Duration.ofMinutes(5))
//    stats.removeIf { it.isBefore(cutoffTime) }
//  }
//
//  /**
//   * Get current network statistics
//   */
//  fun getCurrentStats(): NetworkStats {
//    return stats.values.lastOrNull() ?: NetworkStats()
//  }
//
//  /**
//   * Get average read rate over the last minute
//   */
//  fun getAverageReadRate(): Double {
//    val oneMinuteAgo = Instant.now().minus(Duration.ofMinutes(1))
//    val recentStats = stats.values.filter { it.timestamp.isAfter(oneMinuteAgo) }
//    if (recentStats.isEmpty()) return 0.0
//
//    return recentStats.map { it.readRate }.average()
//  }
//
//  /**
//   * Get the number of errors in the last minute
//   */
//  fun getRecentErrorCount(): Int {
//    val oneMinuteAgo = Instant.now().minus(Duration.ofMinutes(1))
//    return stats.values.count {
//      it.timestamp.isAfter(oneMinuteAgo) && it.consecutiveErrors > 0
//    }
//  }
//
//  private fun logNetworkConditionChange(newCondition: NetworkCondition, stats: NetworkStats) {
//    logger.info(
//      "Network condition changed: {} -> {}, Read rate: {} KB/s, Jitter: {}ms, Tags: {}, Errors: {}",
//      _networkCondition.value,
//      newCondition,
//      String.format("%.2f", stats.readRate / 1024),
//      String.format("%.2f", stats.jitter),
//      tagCount,
//      stats.consecutiveErrors
//    )
//  }
//
//  private fun isPoorCondition(stats: NetworkStats): Boolean {
//    return stats.packetLoss > 5.0 ||
//            stats.consecutiveErrors >= degradationThreshold ||
//            stats.jitter > 100 ||
//            readRate < 1024 ||
//            isReadStalled()
//  }
//
//  private fun isGoodCondition(stats: NetworkStats): Boolean {
//    return stats.packetLoss < 1.0 &&
//            stats.consecutiveErrors == 0 &&
//            stats.jitter < 30 &&
//            readRate > 102400 &&
//            !hasAbnormalTimestampGaps()
//  }
//}