#if canImport(UIKit)
import Foundation

/// Runs `work` on the main actor after the current main-queue work, in
/// submission order (unlike separate Tasks).
func markitOnMain(_ work: @escaping @MainActor @Sendable () -> Void) {
    DispatchQueue.main.async {
        MainActor.assumeIsolated { work() }
    }
}
#endif
