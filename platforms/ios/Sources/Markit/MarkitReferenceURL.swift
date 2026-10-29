import Foundation

extension MarkitReference {
    static let urlScheme = "markit-reference"

    /// The link that attributed text carries for this reference, so hosts that
    /// only see links (for example SwiftUI `Text`) can recover it with `init?(url:)`.
    public var url: URL {
        var components = URLComponents()
        components.scheme = Self.urlScheme
        components.host = "open"
        // Encode everything but unreserved ASCII, so targets may hold & = + ? #.
        let unreserved = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~/")
        func encode(_ value: String) -> String { value.addingPercentEncoding(withAllowedCharacters: unreserved) ?? "" }
        components.percentEncodedQueryItems = [URLQueryItem(name: "kind", value: encode(kind)), URLQueryItem(name: "target", value: encode(target))]
        return components.url ?? URL(string: "\(Self.urlScheme)://open")!
    }

    /// Reads a reference link produced by Markit; nil for every other URL.
    public init?(url: URL) {
        guard url.scheme == Self.urlScheme,
              let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems,
              let target = items.first(where: { $0.name == "target" })?.value else { return nil }
        self.init(kind: items.first(where: { $0.name == "kind" })?.value ?? "file", target: target)
    }
}
