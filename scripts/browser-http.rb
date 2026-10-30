#!/usr/bin/env ruby
require 'net/http'
require 'uri'

module BrowserHttp
  class FetchError < StandardError; end
  MAX_BODY = 2 * 1024 * 1024
  CHUNK = 424
  # Callback receives kind, status and bytes; false means the browser cancelled.
  def self.fetch(url)
    uri = URI.parse(url)
    raise FetchError, 'Expected an HTTP(S) URL without credentials' unless uri.is_a?(URI::HTTP) && uri.host && !uri.userinfo
    http = Net::HTTP.new(uri.host, uri.port)
    http.use_ssl = uri.scheme == 'https'
    http.verify_mode = OpenSSL::SSL::VERIFY_PEER if http.use_ssl?
    http.open_timeout = 15
    http.read_timeout = 30
    http.write_timeout = 15
    http.max_retries = 0
    http.start do |connection|
      request = Net::HTTP::Get.new(uri.request_uri, 'User-Agent' => 'NetSurf/3.11 (Reflex USB)', 'Accept' => '*/*')
      connection.request(request) do |response|
        status = Integer(response.code)
        if [301, 302, 303, 307, 308].include?(status)
          location = response['location']
          raise FetchError, 'Redirect lacks Location' unless location
          target = URI.join(uri, location).to_s
          raise FetchError, 'Redirect URL too long' if target.bytesize > CHUNK
          yield 5, status, target.b
          return
        end
        if response['content-type']
          header = "Content-Type: #{response['content-type']}\r\n".b
          raise FetchError, 'Content-Type too long' if header.bytesize > CHUNK
          return unless yield 1, status, header
        else
          return unless yield 1, status, "HTTP/1.1 #{status}\r\n".b
        end
        length = 0
        response.read_body do |bytes|
          length += bytes.bytesize
          raise FetchError, 'Response exceeds 2 MiB' if length > MAX_BODY
          at = 0
          while at < bytes.bytesize
            part = bytes.byteslice(at, CHUNK)
            return unless yield 2, status, part.b
            at += part.bytesize
          end
        end
        yield 3, status, ''.b
      end
    end
  rescue FetchError, Net::HTTPError, IOError, SocketError, SystemCallError, Timeout::Error, OpenSSL::SSL::SSLError, URI::InvalidURIError => error
    yield 4, 0, error.message.encode('ASCII', invalid: :replace, undef: :replace).byteslice(0, CHUNK).b
  end
end
