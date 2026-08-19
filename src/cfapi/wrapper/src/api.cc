#include "api.h"

namespace
{
class RequestGuard
{
public:
    explicit RequestGuard(cfapi::Session &session)
        : session(session), request(&session.createRequest())
    {
    }

    ~RequestGuard()
    {
        session.freeRequest(*request);
    }

    RequestGuard(const RequestGuard &) = delete;
    RequestGuard &operator=(const RequestGuard &) = delete;

    cfapi::Request &get()
    {
        return *request;
    }

private:
    cfapi::Session &session;
    cfapi::Request *request;
};
} // namespace

PreparedQueryXrefWrap::PreparedQueryXrefWrap(cfapi::Session &session,
                                             const std::string &src_id,
                                             const std::string &symbol,
                                             bool include_symbol)
    : session(&session), request(&session.createRequest()), prepared_tag(0)
{
    request->clearRequest();
    request->setCommand(cfapi::QUERYXREF);
    request->add(cfapi::ENUM_SRC_ID, src_id);
    if (include_symbol)
    {
        request->add(cfapi::SYMBOL_TICKER, symbol);
    }
    prepared_tag = request->generateTag();
}

PreparedQueryXrefWrap::~PreparedQueryXrefWrap()
{
    if (request != nullptr)
    {
        session->freeRequest(*request);
    }
}

std::int64_t PreparedQueryXrefWrap::tag() const
{
    return prepared_tag;
}

std::int64_t PreparedQueryXrefWrap::send()
{
    return session->send(*request);
}

RustMessageEventHandler::RustMessageEventHandler(std::uintptr_t handler_id)
    : handler_id(handler_id)
{
}

void RustMessageEventHandler::onMessageEvent(const cfapi::MessageEvent &event)
{
    cfapi_rust_on_message_event(handler_id, event);
}

APIFactoryWrap::APIFactoryWrap(const std::string &appName, const std::string &appVersion,
                               bool debug, const std::string &logFileName, std::string usage,
                               const std::string &username, const std::string &password,
                               const cfapi::UserEventHandler &userHandler,
                               const cfapi::SessionEventHandler &sessionHandler)
{
    ptr = cfapi::APIFactory::getInstance();
    ptr->initialize(appName, appVersion, debug, logFileName, usage);
    primaryUser = &ptr->createUserInfo(username, password, const_cast<cfapi::UserEventHandler &>(userHandler));
    session = &ptr->createSession(*primaryUser, const_cast<cfapi::SessionEventHandler &>(sessionHandler));
    // printf("api init done.\n");
}

APIFactoryWrap::~APIFactoryWrap()
{
    cfapi::APIFactory::getInstance()->destroySession(*session);
    cfapi::APIFactory::getInstance()->destroyUserInfo(*primaryUser);
    cfapi::APIFactory::getInstance()->uninitialize();
};

void APIFactoryWrap::setSessionConfigInt(cfapi::SessionConfig::Parameters param, long value)
{
    cfapi::SessionConfig &sessionConfig = (*session).getSessionConfig();
    sessionConfig.set(param, value);
};

void APIFactoryWrap::setSessionConfigBool(cfapi::SessionConfig::Parameters param, bool value)
{
    cfapi::SessionConfig &sessionConfig = (*session).getSessionConfig();
    sessionConfig.set(param, value);
};

void APIFactoryWrap::setGlobalConnectionConfig(bool backup, bool compression,
                                               bool conflation_indicator, long conflation_interval,
                                               long read_timeout, long connection_timeout, long connection_retry_limit,
                                               long queue_size, long blocking_connection_time_limit,
                                               long conflation_type, long jit_conflation_threshold_percent)
{
    cfapi::ConnectionConfig &connectionConfig = (*session).getConnectionConfig();
    connectionConfig.set(cfapi::HostConfig::BACKUP_BOOL, backup);
    connectionConfig.set(cfapi::HostConfig::COMPRESSION_BOOL, compression);
    connectionConfig.set(cfapi::HostConfig::CONFLATION_INDICATOR_BOOL, conflation_indicator);
    connectionConfig.set(cfapi::HostConfig::CONFLATION_INTERVAL_LONG, conflation_interval);
    connectionConfig.set(cfapi::HostConfig::READ_TIMEOUT_LONG, read_timeout);
    connectionConfig.set(cfapi::HostConfig::CONNECTION_TIMEOUT_LONG, connection_timeout);
    connectionConfig.set(cfapi::HostConfig::CONNECTION_RETRY_LIMIT_LONG, connection_retry_limit);
    connectionConfig.set(cfapi::HostConfig::QUEUE_SIZE_LONG, queue_size);
    connectionConfig.set(cfapi::HostConfig::BLOCKING_CONNECTION_TIME_LIMIT_LONG, blocking_connection_time_limit);
    connectionConfig.set(cfapi::HostConfig::CONFLATION_TYPE_LONG, conflation_type);
    connectionConfig.set(cfapi::HostConfig::JIT_CONFLATION_THRESHOLD_PERCENT_LONG, jit_conflation_threshold_percent);
};

void APIFactoryWrap::setConnectionConfig(std::string &host_info, bool backup,
                                         bool compression, bool conflation_indicator, long conflation_interval,
                                         long read_timeout, long connection_timeout, long connection_retry_limit,
                                         long queue_size, long blocking_connection_time_limit,
                                         long conflation_type, long jit_conflation_threshold_percent)
{
    cfapi::ConnectionConfig &connectionConfig = (*session).getConnectionConfig();
    cfapi::HostConfig &hostConfig = connectionConfig.getHostConfig(host_info);
    hostConfig.set(cfapi::HostConfig::BACKUP_BOOL, backup);
    hostConfig.set(cfapi::HostConfig::COMPRESSION_BOOL, compression);
    hostConfig.set(cfapi::HostConfig::CONFLATION_INDICATOR_BOOL, conflation_indicator);
    hostConfig.set(cfapi::HostConfig::CONFLATION_INTERVAL_LONG, conflation_interval);
    hostConfig.set(cfapi::HostConfig::READ_TIMEOUT_LONG, read_timeout);
    hostConfig.set(cfapi::HostConfig::CONNECTION_TIMEOUT_LONG, connection_timeout);
    hostConfig.set(cfapi::HostConfig::CONNECTION_RETRY_LIMIT_LONG, connection_retry_limit);
    hostConfig.set(cfapi::HostConfig::QUEUE_SIZE_LONG, queue_size);
    hostConfig.set(cfapi::HostConfig::BLOCKING_CONNECTION_TIME_LIMIT_LONG, blocking_connection_time_limit);
    hostConfig.set(cfapi::HostConfig::CONFLATION_TYPE_LONG, conflation_type);
    hostConfig.set(cfapi::HostConfig::JIT_CONFLATION_THRESHOLD_PERCENT_LONG, jit_conflation_threshold_percent);
};


void APIFactoryWrap::registerMessageEventHandler(const cfapi::MessageEventHandler &messageHandler)
{
    (*session).registerMessageEventHandler(&const_cast<cfapi::MessageEventHandler &>(messageHandler));
};


void APIFactoryWrap::registerRustMessageEventHandler(std::uintptr_t handler_id)
{
    rustMessageHandler = std::make_unique<RustMessageEventHandler>(handler_id);
    (*session).registerMessageEventHandler(rustMessageHandler.get());
};

void APIFactoryWrap::registerStatisticsEventHandler(const cfapi::StatisticsEventHandler &statsEH, int interval)
{
    (*session).registerStatisticsEventHandler(&const_cast<cfapi::StatisticsEventHandler &>(statsEH), interval);
};

std::int64_t APIFactoryWrap::sendRequest(const std::string &src_id, const std::string &symbol, cfapi::Commands command)
{
    RequestGuard request_guard(*session);
    cfapi::Request &req = request_guard.get();
    req.clearRequest();
    req.add(cfapi::ENUM_SRC_ID, src_id);
    req.add(cfapi::SYMBOL_TICKER, symbol);
    req.setCommand(command);
    // std::int64_t ret = ;
    return (*session).send(req);
};

std::unique_ptr<PreparedQueryXrefWrap> APIFactoryWrap::prepareQueryXref(
    const std::string &src_id, const std::string &symbol, bool include_symbol)
{
    return std::make_unique<PreparedQueryXrefWrap>(*session, src_id, symbol, include_symbol);
}

std::int64_t APIFactoryWrap::sendUserFilterTokens(const std::string &src_id, const std::string &token_numbers_csv)
{
    RequestGuard request_guard(*session);
    cfapi::Request &req = request_guard.get();
    req.clearRequest();
    req.setCommand(cfapi::SELECTUSERFILTERTOKENS);
    req.add(cfapi::ENUM_SRC_ID, src_id);

    std::size_t start = 0;
    while (start < token_numbers_csv.size())
    {
        const std::size_t end = token_numbers_csv.find(',', start);
        const std::string token = token_numbers_csv.substr(start, end - start);
        if (!token.empty())
        {
            req.add(cfapi::CTF_TOKEN_NUM, token);
        }
        if (end == std::string::npos)
        {
            break;
        }
        start = end + 1;
    }
    return (*session).send(req);
};

std::int64_t APIFactoryWrap::sendCommand(cfapi::Commands command)
{
    RequestGuard request_guard(*session);
    cfapi::Request &req = request_guard.get();
    req.clearRequest();
    req.setCommand(command);
    return (*session).send(req);
};
bool APIFactoryWrap::startSession()
{
    std::string failReason;
    bool ret = (*session).start(failReason);
    if (!ret)
    {
        printf("session could not be established: %s\n", failReason.c_str());
    }
    return ret;
};

// const cfapi::Session& APIFactoryWrap::getSession() {
//     return *session;
// }

// cfapi::Session *APIFactoryWrap::getSession()
// {
//     return session;
// }

void *GetEventReader(const cfapi::MessageEvent &event)
{
    return &event.getReader();
};

// const cfapi::DateTime GetDatetime(cfapi::MessageReader &reader)
// {
//     return reader.getValueAsDateTime();
// };

void *GetDate(const cfapi::MessageReader &reader)
{
    return &((const_cast<cfapi::MessageReader &>(reader)).getValueAsDateTime().date());
};

void *GetTime(const cfapi::MessageReader &reader)
{
    return &((const_cast<cfapi::MessageReader &>(reader)).getValueAsDateTime().time());
};
