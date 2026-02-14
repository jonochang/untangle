require_relative "user"

class Post
  def author
    User.new
  end
end
