require_relative 'config'

class Server
  def start
    Config.load
  end
end
